//! Deflate y zlib propios (RFC 1950 y 1951), para guardar y leer PNG
//! comprimidos sin dependencias.
//!
//! **Comprimir**: LZ77 con cadenas de hash (ventana de 32 KB, coincidencias
//! de 3 a 258 bytes, se mira si en el siguiente byte empieza una mas larga
//! antes de quedarse con una) y bloques con Huffman dinamico: por bloque se
//! cuentan los simbolos y se arman codigos de largo minimo (Huffman) con tope
//! de 15 bits (si alguno pasa, las frecuencias se parten a la mitad y se
//! vuelve a armar). Los largos de los codigos van a su vez comprimidos con
//! las repeticiones 16, 17 y 18.
//!
//! **Descomprimir**: los tres tipos de bloque (guardado, Huffman fijo y
//! dinamico); cada codigo se lee de una tabla de 2^15 entradas indexada con
//! los siguientes 15 bits (simbolo y largo), asi cada simbolo cuesta una
//! busqueda.

/// Comprime `data` en formato zlib.
pub fn compress(data: &[u8]) -> Vec<u8> {
    let mut out = BitWriter::default();
    // CMF: deflate con ventana de 32 KB; FLG: nivel maximo y el chequeo.
    out.bytes.extend_from_slice(&[0x78, 0xDA]);
    let tokens = lz77(data);
    if tokens.is_empty() {
        // Un bloque fijo vacio.
        out.bits(1, 1);
        out.bits(1, 2);
        let (codes, lens) = canonical(&fixed_lengths().0);
        out.code(codes[256], lens[256]);
    } else {
        let blocks: Vec<&[Token]> = tokens.chunks(1 << 16).collect();
        for (i, block) in blocks.iter().enumerate() {
            write_block(&mut out, block, i + 1 == blocks.len());
        }
    }
    out.flush();
    out.bytes.extend_from_slice(&adler32(data).to_be_bytes());
    out.bytes
}

/// Descomprime un flujo zlib.
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < 6 || data[0] & 0x0F != 8 || (u16::from(data[0]) << 8 | u16::from(data[1])) % 31 != 0 {
        return Err("zlib: cabecera invalida".into());
    }
    if data[1] & 0x20 != 0 {
        return Err("zlib: diccionario no soportado".into());
    }
    let out = inflate(&data[2..])?;
    let end = data.len();
    let expected = u32::from_be_bytes([data[end - 4], data[end - 3], data[end - 2], data[end - 1]]);
    if adler32(&out) != expected {
        return Err("zlib: el Adler-32 no cuadra".into());
    }
    Ok(out)
}

pub fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    // 5552 es el mayor bloque que no desborda `b` antes del modulo.
    for chunk in bytes.chunks(5552) {
        for &x in chunk {
            a += x as u32;
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

// Las tablas de RFC 1951 ------------------------------------------------------

/// Base y bits extra de los largos 257 a 285.
const LEN_BASE: [u16; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LEN_EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
/// Base y bits extra de las distancias 0 a 29.
const DIST_BASE: [u16; 30] = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
const DIST_EXTRA: [u8; 30] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];
/// El orden en que van los largos del codigo de largos.
const CL_ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

/// Los largos del Huffman fijo: literales y largos, y distancias.
fn fixed_lengths() -> (Vec<u8>, Vec<u8>) {
    let lit = (0..288).map(|i| if i < 144 { 8 } else if i < 256 { 9 } else if i < 280 { 7 } else { 8 }).collect();
    (lit, vec![5; 30])
}

// Comprimir -------------------------------------------------------------------

/// Un literal (`dist == 0`) o una copia de `len` bytes desde `dist` atras.
#[derive(Clone, Copy)]
struct Token {
    len: u16,
    dist: u16,
}

const WINDOW: usize = 32 * 1024;
const HASH_BITS: usize = 15;
const MAX_CHAIN: usize = 96;
const GOOD_LEN: usize = 64;

/// LZ77 con cadenas de hash y una mirada adelante (coincidencia perezosa).
fn lz77(data: &[u8]) -> Vec<Token> {
    let n = data.len();
    let mut tokens = Vec::with_capacity(n / 3);
    let mut head = vec![u32::MAX; 1 << HASH_BITS];
    let mut prev = vec![u32::MAX; WINDOW];
    let hash = |i: usize| -> usize {
        let v = (data[i] as u32) << 16 | (data[i + 1] as u32) << 8 | data[i + 2] as u32;
        (v.wrapping_mul(0x9E37_79B1) >> (32 - HASH_BITS)) as usize
    };
    let insert = |head: &mut Vec<u32>, prev: &mut Vec<u32>, i: usize| {
        if i + 2 < n {
            let h = hash(i);
            prev[i % WINDOW] = head[h];
            head[h] = i as u32;
        }
    };
    let longest = |head: &Vec<u32>, prev: &Vec<u32>, i: usize| -> (usize, usize) {
        if i + 2 >= n {
            return (0, 0);
        }
        let max = (n - i).min(258);
        let (mut best, mut best_dist) = (0, 0);
        let mut candidate = head[hash(i)];
        let mut chain = 0;
        while candidate != u32::MAX && chain < MAX_CHAIN {
            let c = candidate as usize;
            if c >= i || i - c > WINDOW {
                break;
            }
            // Primero el byte que mejoraria la mejor (casi siempre falla ahi).
            if data[c + best.min(max - 1)] == data[i + best.min(max - 1)] {
                let mut l = 0;
                while l < max && data[c + l] == data[i + l] {
                    l += 1;
                }
                if l > best {
                    best = l;
                    best_dist = i - c;
                    if l >= max || l >= GOOD_LEN * 4 {
                        break;
                    }
                }
            }
            let next = prev[c % WINDOW];
            if next != u32::MAX && next as usize >= c {
                break;
            }
            candidate = next;
            chain += if best >= GOOD_LEN { 4 } else { 1 };
        }
        if best >= 3 { (best, best_dist) } else { (0, 0) }
    };
    let mut i = 0;
    while i < n {
        let (len, dist) = longest(&head, &prev, i);
        if len >= 3 {
            // Si en el siguiente byte empieza una mejor, este va suelto.
            insert(&mut head, &mut prev, i);
            if len < GOOD_LEN && i + 1 < n {
                let (next_len, _) = longest(&head, &prev, i + 1);
                if next_len > len + 1 {
                    tokens.push(Token { len: data[i] as u16, dist: 0 });
                    i += 1;
                    continue;
                }
            }
            tokens.push(Token { len: len as u16, dist: dist as u16 });
            for k in 1..len {
                insert(&mut head, &mut prev, i + k);
            }
            i += len;
        } else {
            insert(&mut head, &mut prev, i);
            tokens.push(Token { len: data[i] as u16, dist: 0 });
            i += 1;
        }
    }
    tokens
}

/// El codigo de largo (257 a 285) y sus bits extra.
fn len_code(len: usize) -> (usize, u32, u8) {
    let i = LEN_BASE.partition_point(|&b| b as usize <= len) - 1;
    (257 + i, (len - LEN_BASE[i] as usize) as u32, LEN_EXTRA[i])
}

fn dist_code(dist: usize) -> (usize, u32, u8) {
    let i = DIST_BASE.partition_point(|&b| b as usize <= dist) - 1;
    (i, (dist - DIST_BASE[i] as usize) as u32, DIST_EXTRA[i])
}

/// Largos de un Huffman con tope `limit` para las frecuencias `freq`. Con
/// menos de dos simbolos usados se agrega uno (los decodificadores esperan
/// un codigo completo).
fn huffman_lengths(freq: &[u32], limit: u8) -> Vec<u8> {
    let mut f: Vec<u32> = freq.to_vec();
    let used = f.iter().filter(|&&v| v > 0).count();
    if used < 2 {
        for v in f.iter_mut().take(2) {
            *v = (*v).max(1);
        }
    }
    loop {
        let lengths = plain_huffman(&f);
        if lengths.iter().all(|&l| l <= limit) {
            return lengths;
        }
        for v in f.iter_mut().filter(|v| **v > 0) {
            *v = (*v >> 1) | 1;
        }
    }
}

/// Los largos de Huffman sin tope (con un monticulo de nodos).
fn plain_huffman(freq: &[u32]) -> Vec<u8> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let n = freq.len();
    // Nodos: hojas 0..n y despues los internos; `parent` de cada uno.
    let mut parent = vec![usize::MAX; 2 * n];
    let mut heap: BinaryHeap<Reverse<(u64, usize)>> = freq.iter().enumerate().filter(|&(_, &w)| w > 0).map(|(i, &w)| Reverse((w as u64, i))).collect();
    let mut next = n;
    while heap.len() > 1 {
        let Reverse((wa, a)) = heap.pop().unwrap();
        let Reverse((wb, b)) = heap.pop().unwrap();
        parent[a] = next;
        parent[b] = next;
        heap.push(Reverse((wa + wb, next)));
        next += 1;
    }
    (0..n)
        .map(|i| {
            if freq[i] == 0 {
                return 0;
            }
            let (mut d, mut p) = (0u8, parent[i]);
            while p != usize::MAX {
                d += 1;
                p = parent[p];
            }
            d.max(1)
        })
        .collect()
}

/// Los codigos canonicos de unos largos (y los largos).
fn canonical(lengths: &[u8]) -> (Vec<u16>, Vec<u8>) {
    let mut count = [0u16; 16];
    for &l in lengths {
        count[l as usize] += 1;
    }
    count[0] = 0;
    let mut next = [0u16; 16];
    let mut code = 0u16;
    for bits in 1..16 {
        code = (code + count[bits - 1]) << 1;
        next[bits] = code;
    }
    let codes = lengths
        .iter()
        .map(|&l| {
            if l == 0 {
                return 0;
            }
            let c = next[l as usize];
            next[l as usize] += 1;
            c
        })
        .collect();
    (codes, lengths.to_vec())
}

#[derive(Default)]
struct BitWriter {
    bytes: Vec<u8>,
    acc: u64,
    n: u32,
}

impl BitWriter {
    /// `count` bits de `value`, del menos significativo al mas.
    fn bits(&mut self, value: u32, count: u8) {
        self.acc |= (value as u64) << self.n;
        self.n += count as u32;
        while self.n >= 8 {
            self.bytes.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }

    /// Un codigo de Huffman: va del bit mas significativo al menos.
    fn code(&mut self, code: u16, len: u8) {
        let reversed = (code.reverse_bits() >> (16 - len as u32)) as u32;
        self.bits(reversed, len);
    }

    fn flush(&mut self) {
        if self.n > 0 {
            self.bytes.push(self.acc as u8);
            self.acc = 0;
            self.n = 0;
        }
    }
}

/// Un bloque con Huffman dinamico.
fn write_block(out: &mut BitWriter, tokens: &[Token], last: bool) {
    let mut lit_freq = vec![0u32; 286];
    let mut dist_freq = vec![0u32; 30];
    for t in tokens {
        if t.dist == 0 {
            lit_freq[t.len as usize] += 1;
        } else {
            lit_freq[len_code(t.len as usize).0] += 1;
            dist_freq[dist_code(t.dist as usize).0] += 1;
        }
    }
    lit_freq[256] += 1;
    let lit_len = huffman_lengths(&lit_freq, 15);
    let dist_len = huffman_lengths(&dist_freq, 15);
    let hlit = (257..=286).rev().find(|&k| lit_len[k - 1] > 0).unwrap_or(257).max(257);
    let hdist = (1..=30).rev().find(|&k| dist_len[k - 1] > 0).unwrap_or(1).max(1);
    // Los largos seguidos, con sus repeticiones.
    let all: Vec<u8> = lit_len[..hlit].iter().chain(&dist_len[..hdist]).copied().collect();
    let mut runs: Vec<(u8, u8)> = Vec::new(); // (simbolo, extra)
    let mut i = 0;
    while i < all.len() {
        let v = all[i];
        let mut run = 1;
        while i + run < all.len() && all[i + run] == v {
            run += 1;
        }
        let mut left = run;
        if v == 0 {
            while left >= 11 {
                let k = left.min(138);
                runs.push((18, (k - 11) as u8));
                left -= k;
            }
            if left >= 3 {
                runs.push((17, (left - 3) as u8));
                left = 0;
            }
        } else {
            runs.push((v, 0));
            left -= 1;
            while left >= 3 {
                let k = left.min(6);
                runs.push((16, (k - 3) as u8));
                left -= k;
            }
        }
        for _ in 0..left {
            runs.push((v, 0));
        }
        i += run;
    }
    let mut cl_freq = vec![0u32; 19];
    for &(s, _) in &runs {
        cl_freq[s as usize] += 1;
    }
    let cl_len = huffman_lengths(&cl_freq, 7);
    let hclen = (4..=19).rev().find(|&k| cl_len[CL_ORDER[k - 1]] > 0).unwrap_or(4).max(4);
    out.bits(u32::from(last), 1);
    out.bits(2, 2);
    out.bits((hlit - 257) as u32, 5);
    out.bits((hdist - 1) as u32, 5);
    out.bits((hclen - 4) as u32, 4);
    for &k in &CL_ORDER[..hclen] {
        out.bits(cl_len[k] as u32, 3);
    }
    let (cl_codes, _) = canonical(&cl_len);
    for &(s, extra) in &runs {
        out.code(cl_codes[s as usize], cl_len[s as usize]);
        match s {
            16 => out.bits(extra as u32, 2),
            17 => out.bits(extra as u32, 3),
            18 => out.bits(extra as u32, 7),
            _ => {}
        }
    }
    let (lit_codes, _) = canonical(&lit_len);
    let (dist_codes, _) = canonical(&dist_len);
    for t in tokens {
        if t.dist == 0 {
            out.code(lit_codes[t.len as usize], lit_len[t.len as usize]);
        } else {
            let (c, e, eb) = len_code(t.len as usize);
            out.code(lit_codes[c], lit_len[c]);
            out.bits(e, eb);
            let (c, e, eb) = dist_code(t.dist as usize);
            out.code(dist_codes[c], dist_len[c]);
            out.bits(e, eb);
        }
    }
    out.code(lit_codes[256], lit_len[256]);
}

// Descomprimir -----------------------------------------------------------------

struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    acc: u64,
    n: u32,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0, acc: 0, n: 0 }
    }

    /// Llena el acumulador hasta tener al menos 32 bits (o lo que quede).
    #[inline]
    fn refill(&mut self) {
        while self.n <= 56 && self.pos < self.data.len() {
            self.acc |= (self.data[self.pos] as u64) << self.n;
            self.pos += 1;
            self.n += 8;
        }
    }

    #[inline]
    fn bits(&mut self, count: u32) -> Result<u32, String> {
        if count == 0 {
            return Ok(0);
        }
        if self.n < count {
            self.refill();
            if self.n < count {
                return Err("deflate: se acabaron los datos".into());
            }
        }
        let v = (self.acc & ((1u64 << count) - 1)) as u32;
        self.acc >>= count;
        self.n -= count;
        Ok(v)
    }

    /// Lo que queda del byte actual se tira (antes de un bloque guardado).
    fn align(&mut self) {
        let drop = self.n % 8;
        self.acc >>= drop;
        self.n -= drop;
    }
}

/// Una tabla de decodificacion: para cada valor de 15 bits (al reves, como
/// llegan), el simbolo y su largo.
struct Table {
    entries: Vec<(u16, u8)>,
}

const TABLE_BITS: u32 = 15;

impl Table {
    fn new(lengths: &[u8]) -> Result<Self, String> {
        let (codes, _) = canonical(lengths);
        let mut entries = vec![(0u16, 0u8); 1 << TABLE_BITS];
        for (sym, (&code, &len)) in codes.iter().zip(lengths).enumerate() {
            if len == 0 {
                continue;
            }
            if len as u32 > TABLE_BITS {
                return Err("deflate: codigo de mas de 15 bits".into());
            }
            // El codigo llega al reves: sus bits bajos son los primeros.
            let reversed = (code.reverse_bits() >> (16 - len as u32)) as usize;
            let step = 1usize << len;
            let mut i = reversed;
            while i < entries.len() {
                entries[i] = (sym as u16, len);
                i += step;
            }
        }
        Ok(Self { entries })
    }

    #[inline]
    fn decode(&self, r: &mut BitReader) -> Result<u16, String> {
        if r.n < TABLE_BITS {
            r.refill();
        }
        let (sym, len) = self.entries[(r.acc & ((1 << TABLE_BITS) - 1)) as usize];
        if len == 0 || len as u32 > r.n {
            return Err("deflate: codigo invalido".into());
        }
        r.acc >>= len;
        r.n -= len as u32;
        Ok(sym)
    }
}

/// Deflate crudo (sin la envoltura de zlib).
fn inflate(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut r = BitReader::new(data);
    let mut out: Vec<u8> = Vec::with_capacity(data.len() * 3);
    loop {
        let last = r.bits(1)?;
        match r.bits(2)? {
            0 => {
                r.align();
                let len = r.bits(16)? as usize;
                let nlen = r.bits(16)? as usize;
                if len != !nlen & 0xFFFF {
                    return Err("deflate: largo de bloque guardado invalido".into());
                }
                for _ in 0..len {
                    out.push(r.bits(8)? as u8);
                }
            }
            1 => {
                let (lit, dist) = fixed_lengths();
                inflate_block(&mut r, &mut out, &Table::new(&lit)?, &Table::new(&dist)?)?;
            }
            2 => {
                let hlit = r.bits(5)? as usize + 257;
                let hdist = r.bits(5)? as usize + 1;
                let hclen = r.bits(4)? as usize + 4;
                let mut cl_len = [0u8; 19];
                for &k in &CL_ORDER[..hclen] {
                    cl_len[k] = r.bits(3)? as u8;
                }
                let cl = Table::new(&cl_len)?;
                let mut lengths = Vec::with_capacity(hlit + hdist);
                while lengths.len() < hlit + hdist {
                    match cl.decode(&mut r)? {
                        s @ 0..=15 => lengths.push(s as u8),
                        16 => {
                            let prev = *lengths.last().ok_or("deflate: repeticion sin anterior")?;
                            for _ in 0..3 + r.bits(2)? {
                                lengths.push(prev);
                            }
                        }
                        17 => lengths.extend(std::iter::repeat_n(0, 3 + r.bits(3)? as usize)),
                        _ => lengths.extend(std::iter::repeat_n(0, 11 + r.bits(7)? as usize)),
                    }
                }
                if lengths.len() > hlit + hdist {
                    return Err("deflate: largos de mas".into());
                }
                inflate_block(&mut r, &mut out, &Table::new(&lengths[..hlit])?, &Table::new(&lengths[hlit..])?)?;
            }
            _ => return Err("deflate: tipo de bloque invalido".into()),
        }
        if last == 1 {
            return Ok(out);
        }
    }
}

fn inflate_block(r: &mut BitReader, out: &mut Vec<u8>, lit: &Table, dist: &Table) -> Result<(), String> {
    loop {
        let sym = lit.decode(r)? as usize;
        if sym < 256 {
            out.push(sym as u8);
            continue;
        }
        if sym == 256 {
            return Ok(());
        }
        let i = sym - 257;
        if i >= 29 {
            return Err("deflate: largo invalido".into());
        }
        let len = LEN_BASE[i] as usize + r.bits(LEN_EXTRA[i] as u32)? as usize;
        let d = dist.decode(r)? as usize;
        if d >= 30 {
            return Err("deflate: distancia invalida".into());
        }
        let back = DIST_BASE[d] as usize + r.bits(DIST_EXTRA[d] as u32)? as usize;
        if back > out.len() {
            return Err("deflate: distancia mas atras del principio".into());
        }
        let start = out.len() - back;
        if back >= len {
            out.extend_from_within(start..start + len);
        } else {
            // Se copia a si misma (una racha): byte por byte.
            for k in 0..len {
                let b = out[start + k];
                out.push(b);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lo que se comprime vuelve igual: vacio, repetido, texto, ruido y un
    /// degradado como los de las imagenes.
    #[test]
    fn round_trips() {
        let mut noise = Vec::new();
        let mut s = 12345u32;
        for _ in 0..200_000 {
            s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
            noise.push((s >> 16) as u8);
        }
        let gradient: Vec<u8> = (0..300_000u32).map(|i| ((i % 640) / 3 + (i / 1920) % 7) as u8).collect();
        let text = "el cielo de Atitlan y la marimba de la Luna ".repeat(4000).into_bytes();
        for data in [Vec::new(), vec![7u8; 100_000], text, noise, gradient, vec![1, 2, 3]] {
            let packed = compress(&data);
            assert_eq!(decompress(&packed).unwrap(), data);
        }
    }

    /// Comprime: lo repetido queda chico.
    #[test]
    fn repeated_data_shrinks() {
        let data = "Guatemala ".repeat(10_000).into_bytes();
        assert!(compress(&data).len() < data.len() / 50);
    }

    /// Lee un bloque fijo de otro compresor: `zlib.compress(b"abcabcabc")`
    /// de Python.
    #[test]
    fn reads_fixed_huffman() {
        let packed = [120, 156, 75, 76, 74, 78, 4, 35, 0, 17, 61, 3, 115];
        assert_eq!(decompress(&packed).unwrap(), b"abcabcabc");
    }
}
