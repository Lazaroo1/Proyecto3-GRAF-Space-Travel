# Faro: la estrella de neutrones que es púlsar y magnetar

En la órbita de Cementerio ahora está **Faro**, la compañera muerta de la
supergigante: una estrella de neutrones que es púlsar (gira y barre el
sistema con dos haces) y magnetar (su campo de 3×10¹⁴ G tuerce su
magnetosfera, le revienta en estallidos y, con **F**, en una gran llamarada).
Cementerio sigue existiendo solo en la versión chapina, como la Capital.

Está hecha con los números de **1E 1547.0−5408**, un magnetar que también
emite pulsos de radio, y todo lo demás sale de ecuaciones publicadas. La
estrella mide 12 km, sus casquetes polares 130 m, sus haces nacen a 120 km y
los arcos de su magnetosfera suben a 400-700 km, todo en su proporción real.
Se dibuja al doble (`faro::SCALE`: 1 unidad = 500 m, 24 unidades de radio)
para que se vea junto a los planetas; la física no cambia. Su gravedad para la
nave es la de un planeta chico (2 en la superficie): con la de un planeta de
140 km metida en 12, los motores no la sacaban de junto a ella. Los tiempos
también son reales: el giro de 2.07 s, estallidos de milisegundos y una cola
de seis minutos después de la gran llamarada.

## Los números ([scene/faro.rs](src/scene/faro.rs) `Model`)

| Cantidad | Ecuación | Valor |
| --- | --- | --- |
| Masa, radio | (dados) | 1.4 M☉, 12 km |
| Radio de Schwarzschild | rs = 2GM/c² | 4.13 km |
| Compacidad | rs/R | 0.345 |
| Periodo | (1E 1547.0−5408) | 2.0698 s |
| Campo dipolar | (catálogo de magnetares) | 3.2×10¹⁴ G |
| Frenado | B = 3.2×10¹⁹ √(P Ṗ) G → Ṗ | 4.8×10⁻¹¹ s/s |
| Edad característica | τ = P / 2Ṗ | 680 años |
| Energía que pierde | Ė = 4π² I Ṗ / P³, I = 10⁴⁵ g cm² | 2.1×10³⁵ erg/s |
| Cilindro de luz | R_LC = cP / 2π | 98 800 km |
| Casquete de líneas abiertas | sin² θ_pc = R / R_LC | 0.63° (130 m) |

Una prueba compara la edad y Ė con la tabla 1 de Kaspi y Beloborodov (2017):
0.69 mil años y 2.1×10³⁵ erg/s.

## La luz que se curva ([math/neutron.rs](src/math/neutron.rs))

Afuera de una estrella esférica el espacio-tiempo es el de Schwarzschild, así
que la luz sigue la misma ecuación de Binet que en Gargantúa,
u'' + u = (3/2) u² con u = rs/r. La diferencia es que Faro tiene superficie
en R = 2.9 rs, afuera de la esfera de fotones (1.5 rs): cada rayo toca la
superficie o escapa, ninguno da vueltas.

- **Una tabla por distancia**: igual que en Gargantúa, el camino de un rayo
  depende solo del ángulo `s` con la dirección a la estrella. Cuando la
  cámara se mueve, se integran 768 rayos con RK4 (la mitad dentro del disco,
  más densos en el borde) y cada pixel busca su rayo. Si toca la superficie,
  la tabla dice el ángulo ψ del punto de donde salió; si escapa, hacia dónde.
- **Se ve más de media estrella**: el rayo rasante sale de ψ = 121° (con la
  fórmula de Beloborodov 2002, 1 − cos α = (1 − cos ψ)(1 − rs/R); una prueba
  la compara con la integración). Se ve el 76% de la superficie, los dos
  polos magnéticos a la vez, y el disco mide R/√(1 − rs/R) = 14.8 km. La
  esfera que se rasteriza mide eso (por el lapso de la cámara, un poco menos
  de cerca) y su shader decide pixel por pixel qué punto de la estrella se ve.
- **El cielo de fondo** también pasa por la tabla: cerca de Faro las
  estrellas se estiran en arcos alrededor de un anillo de Einstein de
  √(2 rs / d) radianes (5° a 1000 km). Lejos de la tabla se usa la deflexión
  de campo débil, 2rs/b.
- **Corrimiento al rojo**: la temperatura que llega a la cámara es g T, con
  g = √(1 − rs/R) / √(1 − rs/d) para la cámara como observador estático.

## La superficie ([shaders/faro.rs](src/shaders/faro.rs))

- **La corteza que se rompe**: el campo de un magnetar estresa su corteza
  hasta romperla (Thompson y Duncan 1995). La superficie es una red de placas
  violeta oscuro con grietas que brillan, a dos escalas (ruido de Worley con
  el dominio deformado por un ruido de gradiente); las grietas laten cada
  placa a su ritmo y algunos puntos destellan. El borde brilla como una
  atmósfera porque ahí la luz sale rasante (el ángulo de emisión que da la
  lente). **Es color falso**: a millones de kelvin el visible está en la cola
  de Rayleigh-Jeans (hν ≪ kT), donde el color ya no cambia con la
  temperatura, y el ojo vería un blanco azulado parejo. El brillo de las
  grietas sí sigue a la temperatura de abajo.
- **Más fría en el ecuador magnético**: el calor sube por la corteza a lo
  largo del campo (Greenstein y Hartke 1983),
  T⁴ = T_polo⁴ (cos² Θ_B + (κ⊥/κ∥) sin² Θ_B), con cos Θ_B = 2 cos θ / √(1 + 3 cos² θ)
  para un dipolo y κ⊥/κ∥ = 0.05 (con más de 10¹⁰ G es mucho menor que 1).
  La corteza tiene 1.8 MK en el polo y manchas del campo de pequeña escala.
- **Los casquetes del haz de corriente** (Beloborodov 2009): donde las
  líneas torcidas tocan la estrella, las partículas que bajan la calientan.
  El casquete tiene u = sin² θ* (10° en reposo) y
  L = 1.3×10³⁶ B₁₄ R₆ ψ V₉ u² erg/s; su temperatura sale de L = 2 A σ T⁴ con
  A = π R² u (unos 5 MK). Después de la gran llamarada se agranda y se encoge
  despacio, como en los magnetares transitorios. Ahí las grietas pasan de
  azul a rosa y naranja.
- **Los casquetes de las líneas abiertas**: 130 m en cada polo, más
  calientes.
- **La bruma**: alrededor brilla la nebulosa del viento del púlsar (como la
  del Cangrejo, mucho más chica), con densidad 1/(1 + r²/a²)². Su integral a
  lo largo del rayo es exacta: con x = t − t_c y k² = a² + q²,
  ∫ a⁴ dx/(k² + x²)² = a⁴ [x/(2k²(k² + x²)) + atan(x/k)/(2k³)]. Se apaga
  cuando la cámara se mete en ella.

## La magnetosfera torcida ([scene/magnetosphere.rs](src/scene/magnetosphere.rs))

Un magnetar no tiene un dipolo limpio: la corteza tuerce las líneas que salen
de ella y las corrientes que lleva la torsión brillan. El modelo es la familia
autosimilar de Thompson, Lyutikov y Kulkarni (2002):

```text
(1 − μ²) F'' + p (p + 1) F + C F^(1 + 2/p) = 0,   F(1) = 0, F'(1) = −2, F'(0) = 0
B_r = −(B_polo/2) r^−(2+p) F',   B_θ = (B_polo/2) r^−(2+p) p F / sin θ
B_φ = B_θ √(C / (p (1 + p))) F^(1/p)
torsión Δφ = 2 √(C / (p (1 + p))) ∫₀¹ F^(1/p) / (1 − μ²) dμ
```

- Al arrancar se resuelve para 41 valores de p, de 1 (el dipolo) a 0.2, por
  disparo: RK4 desde el polo con la serie F ≈ 2ε − p(p+1)ε²/2 y bisección
  sobre el autovalor C hasta que F'(0) = 0. Una prueba compara C, Δφ y F(0)
  con la tabla de TLK (por ejemplo p = 0.5: C = 0.754, Δφ = 2.06 rad).
- Una línea que nace en μ_f sube hasta r(μ) = (F(μ)/F(μ_f))^(1/p) y su azimut
  avanza con dφ/dμ = −√(C/(p(1+p))) F^(1/p)/(1 − μ²). Para la torsión de
  cada momento se interpola entre las dos familias que la rodean.
- **Qué brilla**: solo las líneas con corriente, las del haz de corriente
  (θ < θ* en Beloborodov 2009), y se dibuja su orilla (la cima a 1/u radios).
  Los pares e± nacen cerca de los dos pies, donde B > 10¹³ G (a menos de unos
  3 radios), suben frenándose porque dispersan los rayos X de la superficie
  (γ ~ 100 B/B_Q) y se aniquilan en la cima (Beloborodov 2013): los pies
  brillan blanco azulado, el medio azul y afuera violeta. Es una visualización:
  esa luz es de rayos X duros, aunque la densidad del plasma podría dar luz
  visible (Kaspi y Beloborodov 2017, §5.4.4).
- Se dibujan tres familias: los arcos cerrados junto a la estrella (pies
  entre 16° y 42°), el abanico de líneas que sale de los polos hacia los
  haces y los arcos grandes del haz de corriente, más tenues.
- Cada línea es un **hilo de luz** ([render/glow.rs](src/render/glow.rs)):
  segmentos que se suman a la imagen con un núcleo de 0.7 px y un halo
  suave de 3.2 px, probados contra la profundidad, en el mismo pase por tiles
  que las partículas. Una cinta opaca tapaba lo de atrás y no podía tener
  halo. Para que las uniones de una polilínea no brillen el doble, solo los
  extremos tienen tapas. Lejos, cuando la estrella mide pocos pixeles, las
  líneas bajan su brillo para no hacer una mancha.
- Los paquetes que suben se muestran diez mil veces más lentos (de verdad
  van casi a la velocidad de la luz) y la descarga parpadea como un relámpago
  continuo (Beloborodov y Thompson 2007).

## Los haces del púlsar

- **Geometría**: radio a 1 GHz desde las líneas abiertas, en un cono hueco
  con núcleo (Rankin 1990, 1993): núcleo de 2.45° P^−½, conos de 4.33° y
  5.75° P^−½. Con P = 2.07 s miden 1.7°, 3.0° y 4.0°. La altura de emisión
  sale de la tangente a la última línea abierta, ρ = (3/2)√(r/R_LC): el cono
  interior nace a 122 km y el exterior a 215 km.
- **El carrusel**: el cono está hecho de 20 chispas que dan vueltas
  alrededor del eje magnético cada 37.4 giros (Ruderman y Sutherland 1975;
  los números de PSR B0943+10), y cada pulso es distinto, como en los
  magnetares.
- **El núcleo nace en la superficie**: es el plasma que sale por las líneas
  abiertas desde el casquete polar; los conos se encienden desde su altura
  de emisión. Las hebras del chorro salen de una tabla de ruido periódico
  (azimut por logaritmo de la altura) que corre hacia afuera: evaluar Perlin
  en cada muestra costaba decenas de milisegundos.
- **Por qué se ven**: la radio no se ve, pero los haces iluminan el viento de
  la supergigante, cuya densidad baja como 1/d² desde ella; el flujo del haz
  baja como 1/r² desde Faro. La luz que llega es ∫ perfil(θ) n(x) / r² a lo
  largo del rayo. Con el cambio de variable t = t_c + q tan u, el 1/r² se
  vuelve exacto (dt/r² = du/q) y bastan 7 muestras. Primero se intersecta el
  doble cono de forma analítica (una cuadrática).
- **El destello**: cuando un haz apunta a la nave se ve la luz directa de la
  región de emisión. Como el haz barre su ancho en uno o dos frames, el
  perfil se promedia en el tiempo de exposición del frame (8 muestras; el
  volumen usa 3) para no perder el pulso entre dos frames.
- **Pulso e interpulso**: el eje magnético está a 82° del de giro y este casi
  perpendicular al plano del sistema, así que los dos haces barren el plano:
  desde casi cualquier lugar se oyen dos pulsos por vuelta.

## La actividad (función pura del tiempo: se rebobina)

- **Estallidos**: un proceso de Poisson determinista (cada ranura de 50 ms
  decide con un hash), con energías dN/dE ∝ E^−1.7 (Göğüş et al. 1999) y
  duraciones lognormales alrededor de 100 ms; subida de 5 ms y caída
  exponencial. Llegan en **tormentas**: cada 90 s puede haber una de 15 a
  25 s. Se encienden las líneas del sector donde ocurren.
- **La gran llamarada (F)**, con la secuencia de Parfrey, Beloborodov y Hui
  (2013):
  1. La corteza tuerce la magnetosfera hasta perder el equilibrio a unos
     3 rad y las líneas se inflan. De verdad tarda años; aquí, 8 s.
  2. Un estallido precursor (el de SGR 1806−20 fue 142 s antes; aquí, 2 s).
  3. El pico de 0.2 s (real), que deslumbra el sistema. Las líneas se abren
     y se vuelven a cerrar al reconectarse.
  4. La bola de fuego: pares a más de 10⁹ K atrapados en las líneas cerradas
     junto a la estrella (Thompson y Duncan 1995). Gira con la estrella (la
     cola pulsa con el periodo) y se evapora capa por capa con
     L = L₀(1 − t/t_evap)³, t_evap = 380 s (Feroci et al. 2001). En los
     primeros 40 s brilla además una corona.
  5. El giro se frena de golpe (antiglitch, ΔP/P = 10⁻⁴, como en SGR 1900+14):
     el panel muestra el periodo nuevo.
  6. La radio se apaga y vuelve después de un minuto, como en PSR J1119−6127
     en 2016.
  7. La magnetosfera queda casi sin torsión y se vuelve a torcer.

## El sonido ([audio.rs](src/audio.rs))

Un golpe seco cada vez que un haz pasa por la nave, que es como suenan los
púlsares en un radiotelescopio: el sintetizador sigue la fase del giro y sabe
en qué fracción de la vuelta pasa cada haz y qué tan adentro del cono está la
nave. Los estallidos chisporrotean. La llamarada truena y su cola suena con
las oscilaciones de la corteza medidas en SGR 1806−20 (18, 26, 30, 92.5, 150
y 625 Hz; Israel et al. 2005, Watts y Strohmayer 2006), más fuertes en
ciertas fases del giro.

## El panel

Cerca de Faro, el HUD muestra las ecuaciones de arriba con sus valores, la
torsión, el casquete caliente, si hay tormenta de estallidos y en qué va la
gran llamarada.

## Qué es real y qué no

| Real | Comprimido o visualizado |
| --- | --- |
| Proporciones de la estrella, sus casquetes, la magnetosfera y los haces | Se dibuja al doble (1 unidad = 500 m); el sistema no está en km: la supergigante y las órbitas son de juego |
| Lente de Schwarzschild exacta, corrimiento al rojo | La corteza es de color falso (el ojo vería un blanco azulado parejo) y la gravedad sobre la nave es la de un planeta chico |
| Giro de 2.07 s, estallidos de ms, pico de 0.2 s, cola de 380 s, antiglitch | La torsión se acumula en 8 s (de verdad, años) y el precursor llega 2 s antes |
| Forma de las líneas (TLK), del haz (Rankin), del carrusel | El plasma brilla en visible y sus paquetes van 10⁴ veces más lentos |
| El cilindro de luz y la estructura de Contopoulos et al. (2024) quedan a 98 800 km, fuera del sistema | Los haces se ven porque iluminan el viento de la supergigante |

## Fuentes

- Beloborodov 2002, ApJ 566, L85: luz que se curva cerca de una estrella de neutrones.
- Beloborodov 2009, ApJ 703, 1044: el haz de corriente y sus casquetes.
- Beloborodov 2013, ApJ 762, 13: el plasma que fluye en los arcos.
- Beloborodov y Thompson 2007, ApJ 657, 967: la descarga.
- Camilo et al. 2007, ApJ 666, L93: radio de 1E 1547.0−5408.
- Contopoulos et al. 2024, arXiv:2404.05408: la ecuación del púlsar.
- Feroci et al. 2001, ApJ 549, 1021: la cola de la gran llamarada.
- Göğüş et al. 1999, ApJ 526, L93: estadística de los estallidos.
- Greenstein y Hartke 1983, ApJ 271, 283: temperatura de la superficie.
- Israel et al. 2005, ApJ 628, L53; Watts y Strohmayer 2006, ApJ 637, L117: oscilaciones.
- Kaspi y Beloborodov 2017, ARA&A 55, 261: los magnetares.
- Parfrey, Beloborodov y Hui 2013, ApJ 774, 92: la magnetosfera que se rompe.
- Rankin 1990, ApJ 352, 247; Rankin 1993, ApJ 405, 285: geometría de los haces.
- Ruderman y Sutherland 1975, ApJ 196, 51: las chispas.
- Thompson y Duncan 1995, MNRAS 275, 255: la bola de fuego.
- Thompson, Lyutikov y Kulkarni 2002, ApJ 574, 332: la magnetosfera torcida.
- Woods et al. 1999, ApJ 524, L55: el antiglitch de SGR 1900+14.

# La versión chapina (V)

- **Un cambio de piel, no otra escena** ([scene/system.rs](src/scene/system.rs)
  `toggle_version`): cada cuerpo guarda lo de la otra versión en un `Look`
  (nombre, material, relieve, anillos, terreno ya generado, atmósfera,
  objetos y si existe) y V lo intercambia con lo suyo, junto con los lugares
  y el cielo. Las órbitas, los radios y la gravedad son los mismos, así que
  la nave no salta. Lo chapín se arma una vez al arrancar (`build_chapin`).
  La Luna de Xelajú es un cuerpo más que solo existe en la versión chapina.
- **Cada planeta en su archivo** ([chapin/](src/chapin)): `mod.rs` es la
  tabla y cada módulo da su `spec`, su relieve (`HeightField`), su shader,
  su vertex shader si mueve vértices (las olas de Atitlán mecen las lanchas),
  sus reglas de reparto, su lugar y los modelos y shaders de sus objetos. El
  comentario de arriba de cada archivo explica sus técnicas.
- **Sin nada de OCASO a la vista**: el cielo es otro cubemap horneado
  (`SkyStyle::Chapin`: otra Vía Láctea, dorada, con su grieta oscura), la
  corona del sol es dorada con rayos (`sky::golden_corona`) y no se dibujan
  los arcos de plasma, la cola de Pira ni las nubes de la corona. Kinich Ajaw
  no late: el latido, el fondo del sintetizador y el final (J) son de OCASO;
  al entrar se guarda la línea de tiempo del final y al volver se recupera.
  Es de día: la luz del sol es de 5200 K y más fuerte, la exposición sube un
  poco y la luz de relleno de las sombras (`Uniforms::fill`) es mayor.
- **San Andrés Xecul** (en los Cuchumatanes, en vez de unas estelas que no
  se reconocían): la iglesia es una malla simple (nave, fachada con remates,
  torres, cúpulas, gradas) y todo el arte de la fachada lo pinta el shader
  con formas de distancia con signo. Los materiales se distinguen por el
  color de vértice comparando cada canal contra 0.5: comparado con `==`, el
  valor interpolado (1.0000001) caía en otro material y salía moteado.
- **La transición** ([render/transition.rs](src/render/transition.rs)): al
  presionar V se guarda la imagen de ese momento y, encima de la nueva, se
  abre una cortina desde el centro: cada pixel compara su distancia al
  centro con el frente (que avanza con un *ease out*) y en la franja del
  borde se dibuja una faja tejida que gira: orillas de picos y, en medio,
  cuadros a lo largo del ángulo con un motivo cada uno (quetzal, volcán,
  mazorca, barrilete y greca escalonada), hechos con formas de distancia con
  signo en las coordenadas del cuadro; al volver, una brasa. Se calcula por
  filas en paralelo sobre la imagen ya terminada. La V también quita el
  título del principio para que no se encime con el de la transición.
- **Las naves botella**: OBJ de Blender (Z arriba, etiqueta hacia +Y)
  girados al cargar para volar con el pico adelante, y dibujados con la boca
  22 grados hacia arriba (`System::ship_fix`): la cámara va atrás y arriba,
  y así ve la etiqueta y no solo el fondo. Miden 1.6 veces la OCASO-1
  (`config::BOTTLE_LENGTH`) y con ellas la cámara de persecución va más
  atrás y más baja (`config::BOTTLE_CHASE`): la botella queda entera a media
  imagen en vez de cortarse abajo. Su shader usa el color de cada
  cara (`Kd`) con vidrio pulido, un borde de Fresnel y la luz que pasa por
  el vidrio a contraluz. Cada una tiene su escape (`effects::Exhaust`), que
  sale del fondo de la botella inclinada.
- **Los vehículos** (el tuk-tuk, la camioneta y el colectivo,
  [scene/obj.rs](src/scene/obj.rs) `load_vehicle`): OBJ de Blender con el
  frente en +X y Z arriba, girados al cargar (`(x, y, z) → (−y, z, −x)`, un
  giro sin espejo) para volar con el frente en −Z; de cerca la versión media
  y de lejos la baja. No traen texturas, así que del `.mtl` se lee todo lo
  que sirve: el color (`Kd`), la luz que emiten (`Ke`) y el brillo (`Ns`),
  que pasa a rugosidad de GGX con `α = √(2 / (Ns + 2))`, rugosidad = √α. Del
  nombre del material sale de qué es cada pieza (`Surface`: pintura, metal,
  vidrio, LED u hule), y eso viaja en la `uv` de sus vértices. El shader
  (`ship::vehicle`) le da a la pintura una capa de laca encima (otra pasada
  de Cook-Torrance sin color, con el 4 % de Fresnel del barniz), al cromo,
  la plata y el oro metal pulido, al vidrio casi negro con el reflejo del
  cielo, y a las LEDs su `Ke` como emisión, que el bloom riega. Los largos
  guardan la proporción de los de verdad (la camioneta un poco achicada) y la
  cámara de persecución se aleja en proporción. Una prueba revisa que los
  faros queden adelante y las llantas abajo.
- **Los lugares de día**: cada lugar está fijo en su planeta y el planeta
  gira, así que en la versión chapina N busca hacia adelante el primer
  momento (hasta dos vueltas y media del planeta) en que el sol está entre
  unos 27 y 53 grados sobre el lugar, y llega ahí (`daylight_at`). Los
  lugares lejos del ecuador tienen temporada de noche polar (San Andrés
  Xecul, cerca del polo de los Cuchumatanes, se quedaba de noche entre los
  minutos 2 y 12): si en esas vueltas no amanece, se sigue buscando en el
  año del planeta.
- **La Torre del Reformador** ([chapin/capital.rs](src/chapin/capital.rs)):
  hecha a partir de una foto. Cuatro patas de celosía que se abren mucho
  abajo (medio ancho `2 + 4 (1 - y/8)^1.8`) con un arco grande de celosía en
  cada cara (por los del frente y de atrás pasa la 7a Avenida); a los lados,
  alas más bajas con su arco y dos círculos; el primer nivel con una
  torrecita en cada esquina; el fuste recto que se afila, con un círculo en
  cada cara de cada tramo; arriba la campana, la linterna y el faro. Cada
  viga de celosía son dos cuerdas unidas en zigzag (`girder`) y los círculos
  son tubos (`hoop`). Acero galvanizado gris oscuro.
- **Los volcanes** ([chapin/fuego.rs](src/chapin/fuego.rs),
  [chapin/atitlan.rs](src/chapin/atitlan.rs)): además de las barrancas
  grandes, el relieve del Fuego tiene surcos: ruido en un círculo alrededor
  del eje del cono (periódico en el ángulo) que serpentea al bajar; el fondo
  va donde el ruido cruza por cero y deja aristas en la silueta. El shader
  agrega surcos más finos solo en la normal: deriva su perfil con
  diferencias finitas en el ángulo e inclina la normal de lado, y se apagan
  cuando miden menos de unos pixeles. La ladera va por franjas torcidas con
  ruido: cumbre de ceniza oscura con los surcos rellenos de ceniza gris,
  zacate y bosque de copas (celdas de Worley). Las barrancas del Fuego
  cortan el bosque como franjas grises, como se ven desde Antigua; en los
  dormidos el bosque sube más. En Atitlán se quitó la franja de nubes
  pintada en las cimas, que se leía como nieve: el bosque sube casi hasta la
  punta y solo esta es pedregal.
- **La columna de ceniza** ([chapin/plume.rs](src/chapin/plume.rs)): un
  volumen en la pasada de volúmenes, como el vapor de Pira. Cada explosión
  del Fuego (las mismas que encienden el cráter y tiran bombas) suelta una
  bocanada que sube rápido, se frena, se infla y el viento se la lleva de
  lado; abajo, un tallo de gas une el cráter con ellas. La densidad es la
  de la bocanada más cercana, con el borde comido por un fbm que sube con
  ella (la coliflor), y la luz es la de la estrella sobre el lado que le da,
  más el cielo; recién salida, la base brilla al rojo. Las bocanadas y la
  esfera que las envuelve se calculan una vez por frame, y cada rayo
  primero ve qué bocanadas cruza y solo marcha ese tramo: así cuesta unos
  9 ms cuando llena media pantalla y casi nada en lo demás.
- **La lava** ([chapin/fuego.rs](src/chapin/fuego.rs) `fire`): una costra
  negra que se resquebraja en placas (celdas de Worley que bajan con la
  colada) y, por las grietas y por el canal abierto junto a la boca, la
  lava al rojo con su temperatura de cuerpo negro. Ladera abajo se enfría:
  placas más grandes, grietas más finas y rojas. La costra es suelo
  iluminado, no luz, y la colada tiene cuerpo en el relieve (rellena el
  fondo de su barranca).
- **Los pueblos**: Antigua tiene árboles en los patios, cafetales afuera,
  camionetas y un mercado; en los valles del Fuego se reparten aldeas y
  árboles de sombra. Xecul es un pueblo: plaza empedrada y calles de tierra
  pintadas por el shader en los mismos ejes con que se colocan las casas
  (las hileras de Antigua, `fuego::row`), el mercado frente a la iglesia,
  pinos y camionetas. Los **puestos** son un objeto nuevo: postes, mesa con
  un corte típico, montones de fruta y verdura y un toldo de lona que deja
  pasar la luz; el shader pone a cada uno su lona y lo que vende.
- **La capital**: la Torre del Reformador mide ahora 37.5 (con más tramos
  en el fuste) y es lo más alto del centro. Alrededor hay más cuadras y, en
  algunas, **torres de oficinas** (otro objeto nuevo): basamento de concreto
  con vitrinas y un muro cortina de vidrio con antepechos y parteluces que
  refleja un cielo de día con Fresnel; de noche, oficinas encendidas. Las
  cuadras de la 7a Avenida quedan sin torres para no tapar la vista, y no
  se construye donde un barranco cruza la cuadra (ahí la cuenta que pone
  cada objeto en la cuadrícula no converge).
- **El HUD** ([hud.rs](src/hud.rs)): la letra es Segoe UI (la de Windows)
  cargada desde memoria con las tildes, un atlas por tamaño, dibujada sin
  escalar y en pixeles enteros para que se vea nítida; si no está, usa la de
  raylib sin tildes. Paneles redondeados con las teclas en cajitas; los
  avisos (como el de J en la chapina) salen en una píldora abajo al centro,
  lejos del panel de vuelo, y suben si la ayuda está abierta. El
  reproductor dibuja sus botones con triángulos, una barra con perilla y una
  portada con un ecualizador que se mueve mientras suena. H vuelve junto a
  Pira (además de Inicio) y la fila de Y dice qué hace: la ruta predicha.
  Los nombres de los planetas que caen bajo el panel de vuelo no se
  dibujan.
- **El reproductor** ([player.rs](src/player.rs)): raylib solo decodifica y
  reproduce. Los archivos se leen con Rust y se le pasan desde memoria,
  porque en Windows raylib abre rutas con la página de códigos ANSI y una
  ruta con tildes no se abriría. Empieza con la canción de `primera.txt` y
  luego las revuelve (Fisher-Yates con una semilla del reloj); al terminar
  la vuelta las revuelve otra vez sin repetir de entrada la última. Cuando
  a una canción le quedan 3.5 s empieza la siguiente: suenan dos streams a
  la vez, uno baja con un coseno y el otro sube con un seno (potencia
  constante, sin hueco de volumen en medio). La marimba procedural
  ([marimba.rs](src/marimba.rs)) es la última pista de cada vuelta y la toca
  el sintetizador; sola no entra al shuffle porque no se acaba nunca.

# Los minijuegos

Cada planeta va a tener su minijuego ([minijuegos/](src/minijuegos)). Por
ahora están el de la Luna de Xelajú, **la marimba de la Luna**; los dos de
la capital, **la carrera del Periférico** (en su anillo) y **los shucos del
Reformador** (en el suelo); el de Antigua, **la Camioneta Surfers**; y el de
San Andrés Xecul, **los barriletes**.

## El marco común ([minijuegos/mod.rs](src/minijuegos/mod.rs))

- **Se ofrece cerca de su planeta**: `available` mira el cuerpo más cercano
  y la altura de la nave. El HUD pone «Enter · …» abajo al centro y Enter lo
  abre. En OCASO la Luna de Xelajú no existe, así que ahí no se ofrece.
- **El mundo se congela**: al abrir un minijuego, la última imagen del render
  pasa a ser el fondo (`lienzo::backdrop`). Se difumina con tres pasadas de
  caja por filas y por columnas, que juntas aproximan una gaussiana (la suma
  de tres variables uniformes ya tiene forma de campana). Después se
  oscurece, se desatura un poco y lleva viñeta. La simulación no avanza
  mientras se juega. Cada frame se copia el fondo y el juego se pinta encima,
  en paralelo por filas: 3-5 ms en vez de los 50-140 del 3D, así que corre a
  los fps de la pantalla. Eso importa en un juego de ritmo, porque el teclado
  se lee una vez por frame.
- **Su propio audio**: un segundo stream de raylib con bloques de 512
  muestras (11.6 ms), contra los 2048 del sintetizador del vuelo. Mientras
  se juega, el sintetizador del vuelo calla y el reproductor de música se
  pausa.
- **El texto encima** (`Overlay`): el juego dice qué texto, paneles y barras
  van y dónde. `hud::minigame` los dibuja con la misma letra y paneles del
  HUD, con sombra para que se lean sobre cualquier fondo. La lógica no sabe
  de raylib, y por eso se puede probar y capturar sin ventana
  (`--captura-marimba`).
- **Récords** en `minijuegos.txt` (líneas `nombre valor`), junto con el
  ajuste de tiempo.

## La marimba de la Luna ([minijuegos/marimba.rs](src/minijuegos/marimba.rs))

**La partitura es la del son.** `marimba.rs` se partió en dos:
`Instrument`, que sabe golpear una tecla y dar muestras (las teclas
modales, el charleo, el toc y la sala), y `Marimba`, el secuenciador que
toca el son solo. La marimba que suena en el vuelo salió idéntica, muestra
por muestra (se comparó el WAV de antes y de después). `written()` da la
partitura sin humanizar, con el papel de cada golpe (`Role`): bajo, octava,
centro, melodía, segunda voz, redoble o adorno. El juego reparte esos
papeles. En *Bajo* quien juega lleva las notas del bajo (y la octava suena
con la fundamental) y el resto es de la marimba sola. En *Tiple* lleva la
melodía; la segunda voz suena con cada nota y el bajo, el centro y los
adornos son de la marimba.

**Tres vueltas que aceleran.** Un mapa de tempo por tramos convierte la
corchea `e` de cada tramo en segundos, `t = t₀ + (e − e₀) / r`, con `r` las
corcheas por segundo del tramo. Hay dos compases de entrada (el bajo y el
centro solos), las tres vueltas a 0.9, 1.05 y 1.2 veces el tempo del son
(119, 139 y 158 negras con puntillo por minuto) y un acorde final en Sol
con el tiple redoblando compás y medio.

**Los carriles siguen la melodía** (`contour`). Cada nota se ubica entre la
más grave y la más aguda de una ventana de unas 11 notas a su alrededor:
`carril = round(3 (nota − mín) / (máx − mín))`. Después se corrige contra la
anterior: una nota repetida va en el mismo carril y una distinta en otro,
hacia donde va la melodía (si sube, a la derecha). Solo en la orilla una
nota distinta repite carril (una prueba lo revisa en todo el son). En el
bajo, el carril sale del lugar de la nota entre las del bajo, de la más
grave a la más aguda.

**El reloj sale del audio.** El sintetizador del juego cuenta las muestras
que genera y la canción vive en ese reloj: `t_audio = muestras / 44100 −
preroll`. Lo sintetizado tarda en oírse lo que hay en la cola de raylib (un
bloque y medio en promedio) más el periodo del dispositivo: unos 30 ms.
Por eso lo que se ve va `t = t_audio − 0.03 − ajuste`, donde el ajuste lo
pone quien juega en el menú (audífonos Bluetooth: 150-250 ms). El audio
avanza a saltos de un bloque, así que el reloj visible corre con `dt` y se
corrige hacia ese blanco con `t += (blanco − t)(1 − e^(−8 dt))`. Con un
error mayor a 0.1 s (un tirón) salta directo. Así la ficha llega a la
tecla cuando suena su nota aunque un frame tarde, y los golpes de quien
juega entran al principio del siguiente bloque.

**El juicio.** Un golpe en un carril se compara con la primera ficha que
falta en ese carril: Perfecto a 45 ms, Bien a 90, Ok a 135. Una ficha que
pasa de 135 ms sin golpe es falla. La racha multiplica los puntos (×1 a ×4,
uno más cada 10). El ánimo empieza en 0.6, sube con los aciertos, baja 0.075
con cada falla y, si llega a 0, la marimba sola calla y el público se va. Un
golpe sin ficha suena con una tecla del acorde de Sol, más suave (se puede
tocar libre en el menú).

**El redoble.** En la marimba una nota larga no se sostiene apretando: se
golpea rápido. Cada golpe le suma a la velocidad del modo de la tecla, del
lado en que ya se mueve, lo que perdió desde el golpe anterior:
`fuerza = 1.6 (1 − e^(−Δt / τ))`, con `τ` lo que tarda la fundamental de
esa tecla en caer a 1/e (`refill_after`). Si se redobla lento, cada golpe
repone más; si se deja de redoblar, la nota se apaga sola, que es lo que
pasa en una marimba de verdad. En *Tiple con redobles* las fichas largas
piden machacar la tecla. Durante un redoble, la nota que sigue en el mismo
carril solo se toma con un golpe casi exacto, para que machacar no se la
robe. En *Tiple* el redoble escrito suena solo al acertar la nota.

**La pista en perspectiva, fila por fila.** La pista es un plano visto por
una cámara: a la profundidad `z` (1 en la línea de golpe), la fila es
`y = y_fuga + k / z` y todo mide `1 / z`. Al revés, cada fila de la pantalla
tiene su profundidad `z = k / (y − y_fuga)` y, como las fichas bajan a
velocidad constante, su instante de la canción:
`τ(y) = (z − 1) / (z_lejos − 1) · T`, con `T` lo que tarda una ficha en
bajar. Así que no se proyectan figuras: cada fila pregunta qué ficha, qué
pulso y qué pedazo de la faja de los bordes le toca en su instante. Todo
queda en perspectiva solo y se acorta con la distancia. Para suavizar los
bordes se mide en pixeles: a lo ancho, `x / medio_ancho(z)`; a lo largo,
el tiempo entre `dτ/dy = T k / ((z_lejos − 1)(y − y_fuga)²)`, los segundos
que mide un pixel en esa fila. Con eso cada ficha es una caja redondeada
con distancia con signo en pixeles, con su canto claro, un rombo con su
punto al centro (como un brocado) y su halo. El cuerpo de las notas largas
lleva una greca en zigzag que corre con la canción.

**La marimba de abajo.** Las teclas son de hormigo, más largas en los
graves, con veta de ruido a lo largo, laca cerca de la punta y luz de
arriba a la izquierda. La tecla golpeada baja un poco y vuelve, y al
acertar se aclara sin tapar la veta. Las cuerdas no se ven encima de las
teclas porque pasan por dentro de la madera en sus nodos (a 22.4 % y
77.6 % del largo, donde la tecla no vibra). Se ven en los huecos, de un
nodo al de la tecla vecina, entre postes de bronce. Al frente cuelga el
faldón de tela típica, con pliegues. Encima: chispas y un anillo que se
abre en cada acierto, la luz del carril que sube al apretar la tecla y las
marcas de dónde tiene que caer cada ficha.

**La marimba de la luna** ([chapin/xela.rs](src/chapin/xela.rs)). El lugar
de la Luna (N) tiene una marimba doble de casi 11 de largo
(`PropKind::Marimba`, en tres niveles de detalle). Lleva la fila de teclas
naturales con la punta de quien toca alineada y más largas en los graves,
la fila de alteradas más alta en grupos de dos y tres como un teclado, un
cajón resonador bajo cada natural, el marco, las patas y el faldón. El
shader le da a cada tecla su tono de hormigo. En el faldón borda una
línea de oro, una greca de dientes y una cadena de rombos con su punto,
puntada por puntada. Cuando una puntada mide menos de un par de pixeles,
pinta su promedio. De noche los hilos brillan como los del huipil de la
luna. La luna mide 48 de radio y a 90 de distancia (la de los otros
lugares) ya no se vería, así que se llega a ella desde cerca, con la
mirada bajada.

**Pruebas.** Cada parte tiene todas sus notas en orden y usa los cuatro
carriles. Las vueltas aceleran y los carriles siguen la melodía. Tocando
cada nota a su tiempo sale todo Perfecto en las tres partes (y el audio es
finito, no satura y se oye). Sin tocar nada el público se va y no cuenta
como récord. Un golpe 70 ms antes es Bien, y uno en otro carril no rompe
la racha. Se dibuja sin huecos en el menú y jugando. Los récords se
guardan y se leen, y con N se llega a la marimba y ahí se ofrece (en OCASO
no).

## La carrera del Periférico ([minijuegos/carrera.rs](src/minijuegos/carrera.rs))

**Pseudo-3D, como los juegos de carreras de antes.** La pista es una lista
de tramos de 4 m (594, unos 2.4 km). Cada tramo tiene su curva, que es cuánto
gira el rumbo en él, y su altura. Las curvas se suavizan al entrar y salir y
se escalan para que una vuelta gire exactamente 2π (la pista rodea a la
capital). Desde la cámara se recorren hacia adelante sumando el rumbo
relativo y el corrimiento lateral: `θ += curva` y `x += tramo · (θ + curva / 2)`.
Cada borde se proyecta con `pantalla_x = centro + (x − x_cam) · f / z` y
`pantalla_y = horizonte − (y − y_cam) · f / z`. Los tramos se recorren de
cerca a lejos guardando la fila más alta ya cubierta: lo que cae abajo de
ella está detrás de una loma y no se pinta. Cada fila guarda qué punto de
la pista muestra. La distancia se interpola en `1/z`, que es lo lineal en
pantalla, y así queda correcta en perspectiva. Con eso cada pixel se pinta
en paralelo en metros: el asfalto (ruido de valor), las rayas discontinuas,
la orilla amarilla, el bordillo, los túmulos de los carriles que tocan, la
meta a cuadros, los charcos de luz de los postes y la neblina de lejos.

**Las naves son sprites horneados** ([minijuegos/hornear.rs](src/minijuegos/hornear.rs)).
Dibujar los 28 000 triángulos de cada nave en cada frame costaría lo que
el 3D. En vez de eso, al abrir la carrera un rasterizador propio dibuja
cada modelo una vez desde 15 rumbos (de −70° a 70°, cada 10°), y la propia
desde otros 27 (de −19.5° a 19.5°, cada 1.5°) tal como la ve la cámara, a su
tamaño en pantalla. Con las dos latas del tráfico son 117 sprites en una sola
pasada paralela, en uno a dos segundos.

- La proyección es ortográfica, con buffer de profundidad y funciones de
  borde baricéntricas. La propia, que se ve de cerca, lleva además
  perspectiva: la cola queda igual y lo que está `dz` metros más adelante
  se achica `E / (E + dz)` hacia el centro, con `E` 1.5 veces la distancia
  de la cámara a la cola (como un lente largo). Sin eso una nave larga se
  veía del mismo ancho adelante que atrás, y el ojo la leía agrandada de
  adelante.
- Lleva 2×2 muestras por pixel (la fracción cubierta es el alfa).
- El sombreado es diferido: solo la muestra más cercana se sombrea, una vez.
- La luz es de noche: luna fría, postes naranjas desde abajo y un cielo que
  se refleja en la laca, el cromo y el vidrio. Las piezas salen de
  `obj::Surface`.
- **Las LEDs** se guardan aparte con un núcleo que tira a blanco, un halo
  corto y uno ancho que se suman afuera del contorno, como neón. Cada foco
  guarda en el byte del alfa su patrón según su color: los ámbar corren
  como luces de feria (la fase sale de su posición a lo largo de la nave),
  los magenta, cian y azules respiran, y los rojos de atrás son stops que se
  encienden al frenar. En la pista, el patrón y la fase se traducen a brillo
  con una tabla de 64 entradas por nave, así que animarlas no cuesta nada.
- **Mips**: cada sprite se promedia 2×2 hasta quedar de menos de 8 px (las
  LEDs con mitad promedio y mitad máximo, para que de lejos no se apaguen).
  En la pista se lee el nivel `log2(1 / escala) − 1` con mezcla entre dos
  niveles solo cerca de la mitad: las de lejos salen nítidas y no
  centellean.

En la pista, cada nave usa el sprite del ángulo con que la ve la cámara:
el rumbo de la pista donde está menos el ángulo del rayo, `θ − atan(x / z)`,
sin el volante (una nave girada, vista desde arriba, se ve chueca, como
ladeada). Así en las curvas se le ve el costado.
Se toma el sprite más cercano (mezclar dos vecinos dejaba la nave doble) y se
corre a lo ancho para que su cola caiga donde caería la del rumbo de verdad:
la cola sigue el giro sin saltos. Se escala con `f / z` y se pega con alfa
premultiplicado. Una loma más cercana la tapa: la fila muestra pista más
cerca que ella. Las que pasan pegadas a la cámara son sólidas (se desvanecen
solo a menos de 1.6 m) y se barren a lo ancho con lo que se mueven respecto
a la cámara (4 muestras). Lo que queda detrás de una nave opaca (la pista,
postes, humo, otras naves) no se pinta.

**Las naves con vida.**

- **Peso**: cada nave tiene un resorte de ladeo y uno de rebote por clase.
  Solo el tuk-tuk se ladea, apenas (inclinar el sprite entero se veía como
  si la nave se fuera de lado); un túmulo y la cresta de una loma la hacen saltar. El
  tuk-tuk gira sobre la rueda de afuera. La chela flota: sube y baja.
- **La chela** es de vidrio ámbar: contraluz en la silueta, cielo reflejado
  con Fresnel y cerveza con burbujas adentro. Debajo lleva un anillo
  antigravedad cian y una cáustica ámbar en la pista.
- **El humo negro** sale de las chimeneas de verdad de la camioneta: se
  buscan en la malla (el metal más alto de cada lado en el frente). Cada
  soplo sale con `0.8 v`, el aire lo frena (`e^(−1.8 dt)`), sube cada vez más
  rápido y crece como `r0 + 1.4 edad^0.7`. Se alumbra con el poste de arriba
  y con la ciudad de abajo.
- **El turbo** es distinto por clase: un cono de llama en el escape que
  apunta a la cámara (amarillo y naranja en la camioneta, blanco y magenta
  con anillos de choque en el colectivo, espuma en la chela, brasa y
  chispas en el tuk-tuk).
- **Chispas** balísticas (`g = 9.8`, rebotan con 0.4) al rozar la baranda,
  al chocar y al pasar un túmulo.
- **Luz de la escena sobre la nave**: el poste más cercano la barre de
  arriba y el neón de cada baranda pinta su costado.

**El cielo y la capital** ([minijuegos/ciudad.rs](src/minijuegos/ciudad.rs)).
La pista va a 400 m sobre la Ciudad de Guatemala. Cada columna de la pantalla
es un rayo de una cámara estenopeica; la pista guarda su trazo en metros
(el rumbo integrado), así que la ciudad corre de verdad con las curvas.

- **La ciudad** es una textura de 2048×2048 texeles de 3 m que se repite
  (un cuadro de 6 km), horneada al abrir la carrera en unos 100 ms. Tiene
  zonas de Voronoi con su cuadrícula, cuadra y densidad; barrancos que la
  parten (las curvas de nivel cero de un ruido); calles con un farol cada
  28 m, de sodio o LED según la zona; calzadas con faroles y carros (faros
  blancos de un lado y stops rojos del otro); y La Aurora con su pista
  iluminada y el estadio. Se guarda con gamma 2 y con mips en luz lineal.
- **El rayo contra la esfera** del planeta (radio de 160 km) da dónde pega
  cada pixel. El nivel del mip sale de lo que mide el pixel en el suelo, y
  hacia el horizonte se funde en una neblina violeta.
- **El cielo**: un gradiente con el airglow verde pegado al limbo curvo, una
  banda magenta y un domo de sodio, sumados en modo pantalla al panorama
  del cubemap (con la Vía Láctea plateada). Pasa una estrella fugaz cada 5 a
  15 s.
- **Los volcanes**: Agua, Acatenango (dos picos), Fuego y Pacaya como
  siluetas por columna, con perfil de cono empinado y falda cóncava. El
  Fuego hace erupción cada 6 a 11 s y una justo en el verde: destello,
  ríos de lava, un penacho que dobla el viento y bombas en parábola.

**El asfalto mojado.**

- **Reflejos**: lo que emite en un frame (las LEDs, el neón, los faroles)
  deja una estela que baja por columna (`S = 0.1 L + 0.9 S`). La pista la lee
  en el frame siguiente: cada luz deja una cortina de su color hacia la
  cámara.
- **El cielo reflejado** con Fresnel de Schlick,
  `F = 0.02 + 0.98 (1 − sen α)^5`: de lejos la pista es un espejo.
- **Los postes** dejan su charco con `cos³` de la luz y una raya especular
  que baja hacia la cámara. Cada nave lleva su charco del color de sus LEDs
  y su sombra de contacto, y las calaveras se reflejan al frenar.
- **El material**: losas de 32 m, juntas que destellan, ojos de gato que con
  turbo se estiran en trazadoras, bordillo celeste y blanco con relieve,
  hombro con los rombos de la faja y túmulos con chevrones. Las rayas se
  filtran en caja (la integral de la raya sobre lo que mide el pixel), así
  que de lejos no parpadean.

**La escenografía chapina** ([minijuegos/escenario.rs](src/minijuegos/escenario.rs)).

- **La Torre del Reformador** sobre la meta: celosía de hierro de 66 m con
  la manta a cuadros. Sus focos LED son también el semáforo: se llenan de
  rojo por tercios con la cuenta y se ponen verdes al arrancar.
- **Túneles de arcos de foquitos de feria** en cinco tramos, uno cada 16 m,
  con 41 focos de cinco colores que tiñen la pista.
- **Barriletes gigantes de Sumpango**, horneados al abrir la carrera: rueda
  de papel de china con orilla de colores, greca maya, gajos con los motivos
  de la faja y un sol al centro, vistos a contraluz. Uno cada 200 m y el
  festival, uno cada 48 m antes de la meta.
- **Postes** con tira LED, filo de neón y cono de luz.
- **Cohetes** sobre la capital: uno de ambiente cada 4 a 9 s y cuatro al
  cruzar la meta, de cinco clases (palmera dorada, navideño, patrio, de
  papel de china y crepitante), con su pum que llega con el retraso del
  sonido.

**La luz** ([minijuegos/posproceso.rs](src/minijuegos/posproceso.rs)).
Durante la pasada de filas cada pixel guarda cuánto emite en el byte del
alfa; lo opaco que se pinta encima lo borra, así una nave tapa la baranda.
Lo que emite se extrae a un cuarto de la pantalla con la croma subida (para
que el brillo salga del color de la luz y no blanco) y pasa por la pirámide
de bloom del juego principal: un brillo de dos escalas. El compuesto pone
viñeta (menos donde algo emite), el brillo en modo pantalla, una gradación
de sombras índigo y altas de sodio, curva S y grano. Con la velocidad la
cámara se abre (un resorte) y salen líneas; con el turbo hay más y la
aberración cromática separa los canales en la orilla.

**La física**, a 120 pasos por segundo:

- **Acelerar**: `v += a (1 − v / v_max) dt`. Sin acelerador hay arrastre; el
  freno quita 26 m/s².
- **Fuera del asfalto** la velocidad máxima baja a la mitad, y las barandas
  frenan.
- **De lado**, el volante mueve con el manejo de cada nave (más a más
  velocidad) y la curva empuja hacia afuera con `κ v² · 0.2` (`κ` es la
  curvatura en 1/m). Entrar a fondo a la curva cerrada (radio de 73 m) con la
  camioneta saca más de lo que su volante corrige: hay que frenar.
- **Choques**: rectángulos. De lado se empujan según su masa; por atrás, el
  de atrás baja a la velocidad del de adelante.
- **Túmulos**: pasar uno en su carril a más de 18 m/s quita el 38 % de la
  velocidad (la chela flota y no le hacen nada).
- **Rebufo**: ir de 4 a 28 m atrás de otra nave, en su carril, carga el turbo.
  El turbo da 25 % más de velocidad y 60 % más de empuje por hasta 3 s.
- **Salida perfecta**: acelerar justo en el verde guarda turbo.

**Las rivales y el tráfico.**

- **La IA** apunta al lado de adentro de la curva que viene (la curvatura
  promedio de los próximos 60 m).
- Esquiva lo que va adelante y más lento, y busca un carril sin túmulo.
- No pasa de lo que aguanta la curva más fuerte que tiene enfrente:
  `v = √(2.8 · manejo / κ)`.
- Gasta el turbo en las rectas.
- Tiene un poco de liga: hasta 6 % más rápida si va lejos detrás de quien
  juega y 5 % menos si va muy adelante.
- **El tráfico** son 18 naves lentas de las cuatro clases, y una de cada
  tres es una lata gigante (la Four Loko y la Jägermeister de las naves
  botella) que flota como la chela. Cambian de carril
  de vez en cuando y, si les pitan (H), se hacen a un lado.
- **El humo negro**: las camionetas que aceleran sueltan soplos de sus
  chimeneas que crecen, suben y se apagan. Si la cámara pasa por uno denso,
  se nubla la pantalla.

Con el piloto automático las cuatro clases terminan en 2:14-2:25 (unos 44 s
por vuelta). La camioneta es la más difícil: gana en las rectas y pierde en
las curvas.

**El sonido** también se sintetiza:

- **El motor de cada clase**: el tuk-tuk de dos tiempos son pulsos angostos
  con zumbido, la camioneta un diésel en diente de sierra grave con un sub,
  el colectivo un diente de sierra con su octava abajo, y la chela lleva
  burbujas. Las vueltas suben en cada una de cinco marchas y caen al cambiar.
- **Lo demás**: el viento sube con la velocidad y hay golpes para choques y
  túmulos. El pito es de dos tonos según la nave, más el pitido del
  semáforo y el soplido del turbo.

**Pruebas.**

- La pista cierra: gira 2π y vuelve a su altura, y el mapa cierra.
- Nadie se mueve en el semáforo.
- Con el piloto automático las cuatro clases terminan las tres vueltas en
  tiempos de carrera.
- Un choque por atrás separa a las naves y frena a la de atrás.
- El pito mueve al tráfico de adelante.
- Se dibuja sin huecos antes, durante y después.
- Todo el texto usa letras que la fuente carga.
- El horneado ve una caja de atrás y de lado del tamaño correcto, con la LED
  encendida y su resplandor afuera del contorno.
- La ciudad se repite sin costura y tiene luces y barrancos; el limbo, el
  airglow y los volcanes caen donde deben, y el Fuego da la salida.
- La tabla de tono sigue a la curva, las rayas filtradas dan su integral y
  la pista mojada sale finita y en rango.
- Saltarse lo que tapa una nave opaca da la misma imagen que pintarlo todo.
- Lo que tapa a algo que emite borra su emisión; la estela baja desde lo que
  emite.
- Las chimeneas y el escape salen de la malla; el tuk-tuk se ladea hacia
  afuera; un choque suelta chispas una sola vez; el humo sube, crece y se va.

## Shucos del Reformador ([minijuegos/shucos.rs](src/minijuegos/shucos.rs))

**El juego.** Una noche de 150 s (de las 19:00 a las 03:00) en una carretilla
al pie de la torre. Hay cuatro lugares en el mostrador y los clientes llegan
cada 7 s, o cada 3.4 s en las horas pico: la salida de la U (21:00-22:00, casi
todos estudiantes) y la del estadio (22:30-23:30, aficionados). Después de la
una salen los bolos. Cada cliente trae su pedido y su paciencia (32 s, menos
el agente de la PMT y más el bolo); la barrita del globito se vacía y, si
llega a cero, se va enojado.

- **El shuco** se arma por partes. **1** pone el pan en la tabla. Cada carne
  tiene su lugar en la plancha: su tecla la pone a cocinar y, la segunda
  vez, la pasa al pan. Cada carne tiene su tiempo para quedar lista y para
  quemarse (la salchicha 3.5 y 9.5 s, el chorizo 5.5 y 12 s); la barrita bajo
  la carne lo dice. Lo de encima suma una porción por tecla.
- **El pedido** se revisa al entregarlo: las carnes exactas, ni crudas ni
  quemadas, lo que se pidió (y «bastante» son dos porciones), nada de lo que
  dijo que no, y nada de más si no lo pidió «con todo». Si algo falla te lo
  devuelve diciendo qué («¡Te dije sin cebolla!», «¡El chorizo está
  crudo!») y el shuco se queda en la tabla para arreglarlo.
- **La plata**: el pan cuesta Q1 y cada carne Q3; el shuco se paga por sus
  carnes (el mixto con descuento) más hasta Q5 de propina, según cuánta
  paciencia le quedaba. Lo ganado va a los ahorros (`minijuegos.txt`), con los
  que se compran mejoras: la plancha de hierro (cocina 35 % más rápido), el
  toldo (la gente aguanta 25 % más) y el ayudante (pone el pan solo).

**El dibujo.** Todo en CPU, con figuras de distancia con signo: cajas
redondeadas, discos, segmentos (cápsulas) y arcos. La cobertura de cada pixel
sale de la distancia al borde (`lienzo::cover`), así los bordes quedan
suaves sin muestrear de más, y algunas figuras llevan resplandor
(`exp(−d / alcance)` afuera). El fondo se hornea una vez al abrir (5 ms): el
cielo, los edificios con ventanas al azar, la Torre del Reformador (dos
patas de celosía que suben juntándose con la curva `x = 0.11 w (1 − t)^1.6`,
sus cruces, plataformas, el arco y el faro rojo), la 7a Avenida con el paso
de cebra, los postes de sodio, el toldo de faja y el mostrador de acero
cepillado con la canasta, la plancha, la tabla, los tazones y los botes.
Cada frame se copia el fondo y, por filas en paralelo, se pintan encima los
clientes (arriba del mostrador), las carnes y el shuco (abajo) y el vapor,
el humo y las monedas (en todas). Un frame cuesta de 1 a 2.5 ms.

- **Los clientes** son figuras armadas: el cuerpo, los brazos con las manos
  en el mostrador, el pelo como un disco detrás de la cabeza (se ve su
  orilla) y la cara con ojos y boca (un arco: sonrisa abajo, enojo arriba).
  Cada uno con lo suyo: la mochila y los lentes del estudiante, la gorra y
  el chaleco del agente, el chongo, los aretes y el delantal de la señora, la
  camisola roja, la bufanda y la gorra al revés del aficionado, y la nariz
  roja, el pelo alborotado, la botella y el bamboleo del bolo.
- **Las carnes** cambian de color con la cocción (de cruda a dorada hasta
  estar lista y de ahí a negra), se les marcan las rayas de la parrilla y
  chisporrotean. Lo que se quema echa humo.
- **El shuco** se dibuja por capas como uno de verdad: el pan abierto, las
  carnes, el guacamol, el repollo en tiritas, la cebolla en cuadritos y las
  salsas en zigzag, una línea por porción.

**El sonido**: la plancha chirría (ruido con chasquidos sueltos, más fuerte
con más carnes y con lo quemado), el tráfico de la avenida suena lejos y de
vez en cuando pita una camioneta; hay campanita cuando llega alguien,
moneda al cobrar, zumbido cuando te lo devuelven, y la marimba (el mismo
instrumento de la marimba de la Luna) toca un arpegio con cada entrega buena.

**Pruebas**: los pedidos aceptan lo bien hecho y rechazan con su queja lo
crudo, lo quemado, la cebolla prohibida, el guacamol que no es bastante y lo
que no se pidió; las carnes se cocinan y se queman; una noche entera con el
piloto automático vende al menos 14 sin devoluciones y gana plata; las
mejoras cuestan ahorros; y cada pantalla se dibuja sin huecos con texto que
la fuente carga.

## Camioneta Surfers ([minijuegos/surfers.rs](src/minijuegos/surfers.rs))

**El juego.** Un corredor sin fin por la calle del Arco, en tres carriles de
2.2 m. La velocidad sube de 12 a 24 m/s (`v = 12 + 0.11 t`). El patojo salta
con 8.6 m/s contra una gravedad de 26 m/s² (sube 1.4 m y pasa 0.66 s en el
aire) y rueda 0.62 s; en el aire, abajo lo hace bajar de golpe. La física
corre en pasos de 10 ms, así a toda velocidad nada se atraviesa.

- **La calle se arma por tramos de 28 m**, siempre con un carril libre:
  camionetas (paradas o de frente), alfombras de aserrín (si dejan un carril
  limpio, depende de lo difícil), un lazo de papel picado y carretas de
  helados, tuk-tuks parados, o un respiro con lenes en zigzag. Las alfombras
  que no caen en carriles seguidos se parten en dos (la caja va del primer
  carril al último).
- **Las camionetas paradas** se suben por la escalera de atrás: si se llega
  a la cola desde atrás y en su carril, el patojo sube en 0.25 s y corre en
  la parrilla (a 3 m) hasta que se acaba la camioneta. Las que vienen de
  frente (a 7 m/s) también tienen techo, pero desde el suelo no perdonan.
- **Choques**: cajas contra la caja del patojo (1.7 m de alto, 0.8 rodando).
  La carreta (1.1 m) se pasa saltando; el lazo (de 1.05 a 1.65 m), rodando.
- **Las alfombras**: pisarla en el suelo suelta a los cucuruchos por 7 s;
  pisar otra mientras tanto es perder.
- **Las bombas del Fuego** empiezan a los 12 s y caen cada vez más seguido
  (cada 5 s al principio y cada 2 s con la erupción a todo). Cada una se
  apunta adelante, en un carril, donde va a estar el patojo en 1.8 s; su
  sombra se oscurece y su anillo se enciende en esos 1.6 s, y al caer
  salpica lava y ceniza.

**El dibujo**, todo en CPU, por filas en paralelo. La cámara mira de frente
(a 5.6 m detrás y 3.1 m arriba), así el rayo de cada pixel es `(rx, −ry, 1)`
y `t` es directamente la profundidad. Cada rayo se prueba contra:

- **El suelo** (`t = alto / ry`): el empedrado son piedras redondeadas en
  hileras corridas (una caja redondeada por celda, con su tono al azar) y
  de lejos, cuando un pixel ya mide varias piedras, se promedia; las
  banquetas, las alfombras (orilla, rombos y flores de aserrín, y un manchón
  si se pisaron) y las sombras de las bombas.
- **Las fachadas** (planos `x = ±4.4 m`, `t = (±4.4 − x) / rx`), con la altura
  de cada casa: el color de la casa, el zócalo, la puerta de tableros, las
  ventanas con reja (algunas encendidas), el farol con su luz en la pared y
  el alero de teja.
- **El Arco de Santa Catalina**, cada 420 m: un muro amarillo con su arco,
  las molduras blancas, la hilera de ventanitas, el reloj y la cúpula.
- **Las cajas** de lo que hay en la calle, con la prueba de las placas: el
  rayo entra a la caja por la última placa que cruza y sale por la primera;
  la placa por la que entra dice la cara (la de atrás, el techo o el lado).
  Cada cara se pinta según qué es: la camioneta parada de espaldas con su
  ventana, sus calaveras y la escalera; la que viene con su parabrisas, el
  letrero, los focos y la parrilla del radiador; el techo con la parrilla y
  los bultos; los lados con ventanas y llantas; el tuk-tuk, la carreta con su
  helado pintado y el papel picado (con sus recortes, por donde se ve lo de
  atrás). Solo se prueban las cajas que caen en la fila.
- **El cielo** del atardecer: el Volcán de Agua al fondo (justo detrás del
  Arco), el Fuego a la derecha con el cráter encendido y su penacho de
  ceniza que el viento ladea, y la ceniza que poco a poco tapa todo.

Lo lejano se funde con la neblina (`1 − e^(−t/110)`). Encima, cada pixel
con su profundidad (`figuras.rs`, que ahora comparten con los shucos) van las
monedas que giran, las bombas que caen con su estela, la lava, el polvo y
las chispas, el patojo de espaldas (camisola celeste y blanco, gorra al revés
y su morral típico; corre, salta, sube y rueda hecho bolita) y los
cucuruchos, de túnica morada y capirote, que lo persiguen. Un frame cuesta
unos 7 ms.

**El sonido**: el viento sube con la velocidad, el volcán retumba cada vez
más, los pasos, el salto, la moneda, el pito de dos tonos de la camioneta
que viene, el silbido de cada bomba (que baja de tono hasta caer) y su
explosión, y mientras persiguen los cucuruchos, una marcha de procesión
(bombo y un acorde grave de metales).

**Pruebas**: el rayo entra a la caja por la cara que debe; cada tramo deja un
carril libre; sin saltar la carreta se choca y saltando se pasa; por la
escalera se sube al techo; una segunda alfombra con los cucuruchos atrás es
perder; el piloto automático corre más de 500 m en un minuto y junta lenes;
y cada pantalla se dibuja sin huecos con texto que la fuente carga.

## Barriletes de Xecul ([minijuegos/barriletes.rs](src/minijuegos/barriletes.rs))

**La física**, de lado (x a favor del viento, y arriba), en metros y a 240
pasos por segundo. El barrilete pesa 350 g y tiene 0.6 m².

- **El viento** sopla a unos 8 m/s y crece con la altura (`W (1 + 0.006 y)`),
  con su vaivén (una suma de senos) y ráfagas: suben en 1.2 s (el aviso da
  tiempo de soltar), se quedan y se van, y soplan 35 % más.
- **El aire sobre el barrilete**: con el viento aparente `w = W − v` y
  `q = ½ ρ |w|² A`, la sustentación es `q C_L` perpendicular a `w` (hacia
  arriba) y el arrastre `q C_D` a lo largo de `w`. Para una placa plana con
  ángulo de ataque `α` (lo que se inclina con ← →):
  `C_L = 1.2 sen 2α` (lo más a 45°) y `C_D = 0.08 + 1.2 sen² α`. Más el peso y
  lo que arrastra la cola.
- **El hilo es una restricción**: si el barrilete queda más lejos de la mano
  que el largo del hilo, se le regresa al círculo y se le quita la velocidad
  hacia afuera, `v_r = v · d`. Ese impulso, `m v_r`, entre el paso, es la
  tensión (suavizada en 80 ms). Con 30 m de hilo y viento parejo queda en
  unos 25 N; si pasa de 50 N por un cuarto de segundo, revienta. Jalar
  acorta el hilo a 3 m/s (el barrilete sube contra el viento); soltar lo
  alarga a 6 m/s (queda flojo: ya no tiene de dónde agarrarse y baja).
- **La cola** es una cadena de 14 puntos con Verlet: cada uno sigue su
  inercia, el viento lo empuja y la gravedad lo jala; después se corrigen las
  distancias entre vecinos en tres pasadas.

**Los cortes**: si un tramo de la cola cruza el hilo de otro barrilete
(prueba de cruce de segmentos, con el hilo como polilínea combada si está
flojo), ese hilo revienta y su barrilete se va con el viento; en 9 s ese
patojo eleva otro. Los otros vuelan con su propia física y buscan su altura;
a ratos (unos 8 s de cada 30, después de los primeros 10) atacan: buscan la
altura del nuestro, y si su cola cruza nuestro hilo, nos lo cortan.

**El dibujo**, en CPU por filas: el cielo de la tarde con el sol, las nubes
(ruido de valor en dos escalas que corre con el viento) y los Cuchumatanes en
dos capas (el perfil de cada columna se calcula una vez por frame). Encima,
con las figuras de `figuras.rs` (que ahora tienen polígonos convexos): el
pueblo, la iglesia de San Andrés Xecul (la fachada amarilla con sus columnas
de colores y sus angelitos, las torres y la cúpula), los patojos con sus
latas de hilo, los hilos, las colas con sus moños y los barriletes: un
hexágono de papel de china con seis gajos, el centro, el fleco de la orilla
y las cañas. La cámara sigue al barrilete y se aleja cuando sube.

**El sonido**: el viento y el papel que tiembla con el viento aparente, el
hilo que zumba cuando está tenso, el chasquido cuando revienta, el «zas» y la
porra cuando cortás uno, y la marimba de la fiesta de fondo (la misma
marimba procedural).

**Pruebas**: con viento parejo vuela arriba de la mano, a favor del viento y
con el hilo tenso; sin viento baja; con un ventarrón revienta el hilo; los
segmentos que se cruzan se encuentran; con el piloto automático aguanta la
tarde; y cada pantalla se dibuja sin huecos con texto que la fuente carga.

# Cambios de la sexta revisión

- **Argos reemplaza a Réquiem** (misma órbita, radio 200): un planeta de
  carne con un ojo enorme que **te sigue por todo el sistema**. Réquiem, su
  luna Epitafio, el anillo de ruinas, los monolitos, los picos de roca y la
  nieve se fueron.
- **El ojo se mueve dentro de la carne** ([shaders/argos.rs](src/shaders/argos.rs)):
  no está en el relieve. El material lleva cada frame a dónde mira
  (`ArgosLook`), y el vertex shader (`argos::lift`) mueve cada vértice en la
  dirección radial: cerca del ojo baja la carne hasta una esfera exacta (el
  globo), le sube la córnea y levanta un borde de párpados. El planeta sigue
  girando debajo (orbita y rota como los demás) y la carne pasa por debajo
  del ojo. Para esto `shaders::displace` ahora devuelve un vector: Jardín
  respira sobre la normal y Argos se mueve en radial, porque sobre la normal
  inclinada de los pliegues el globo no quedaba liso y la sombra y el AO
  dibujaban los pliegues encima.
- **Sacadas**: el ojo fija un punto y salta cuando la nave se le sale de ahí
  o cada medio segundo a segundo y medio, como un ojo de verdad; el salto es
  rápido pero con tope de velocidad, así que si la nave está del otro lado
  se ve llegar arrastrándose bajo la carne (`App::stare`, `turn_toward`).
  Parpadea cada 8 s a una hora al azar (función del tiempo). En el silencio
  del final lo cierra, y cuando se abre Gargantúa deja de mirarte y la mira
  a ella.
- **De cerca**: la pupila se cierra y la esclera se inyecta de sangre a
  medida que la nave se acerca. Volando bajo el ojo va **debajo de la
  nave**, un poco adelante de hacia donde apunta (así la pupila queda a la
  vista y no tapada por la nave): se vuela encima del iris, con la pared de
  los párpados en el horizonte. Los ojos chicos de la carne se hunden cuando
  el grande les pasa encima (`argos::rim_distance`).
- **La corona**: sobre los párpados del ojo grande van 28 ojos chicos que se
  mueven con él (se arman cada frame con `argos::rim_point` y los levanta el
  mismo vertex shader de los párpados) y miran hacia adentro, a la nave.
  Desde lejos rodean al ojo; volando encima, te miran desde la pared de
  carne.
- **Iris** en polares alrededor del eje del ojo (anclado al ojo, no a la
  carne): estrías y criptas en el rojo de adentro, la collareta encendida,
  fibras radiales magenta y moradas (ruido con mucha frecuencia a lo largo
  del ángulo y poca a lo largo del radio), hebras lila y el anillo límbico;
  brilla un poco por dentro, así que se ve de noche y desde lejos. La
  **esclera** tiene venas que entran desde los párpados hacia el iris (ruido
  estirado a lo largo del radio, donde cruza por cero) y laten con la
  estrella. La **córnea** es un espejo mojado con un segundo brillo de la luz
  del cielo. La luz de la estrella llega casi sin color.
- **Carne** ([terrain/shapes.rs](src/terrain/shapes.rs) `ArgosField`):
  pliegues redondos donde un ruido torcido cruza por cero, en dos escalas,
  con perfil `(1 - x²)^1.5` (una semicircunferencia tiene pendiente
  infinita en el borde y la malla la dibujaba en dientes de sierra). Granate
  y mojada, crestas claras, surcos negros y venas moradas.
- **Ojos de carne** (`PropKind::Eye`): un globo medio hundido en un bulto
  de carne, con el borde grumoso de la cuenca. Los párpados los dibuja el
  shader sobre el globo: piel fuera de una almendra alrededor del eje de la
  cuenca, con la arruga oscura y la mucosa en el borde, y se juntan en medio
  al parpadear (cada ojo a su ritmo). El iris mira a la nave (su dirección
  se lleva a los ejes del modelo) sin salirse de la almendra. Se reparten con
  ruido en lo alto de los pliegues, en **El nido** (el lugar de Argos para
  la tecla N; cuando llegás, el ojo grande viene debajo de vos y los del
  nido se hunden para dejarlo pasar) y en la corona. Flotan gotas rojas
  cerca de la superficie.
- La sombra del globo se busca un poco arriba de la superficie, sobre la
  normal y más con la luz rasante: sin eso el mapa de sombras hacía franjas
  en la esfera lisa.

# Cambios de la quinta revisión

- **Pira ya no se estira** ([shaders/vapor.rs](src/shaders/vapor.rs)): la
  deformación de la cuarta revisión se veía como una goma. Ahora la marea la
  hace hervir: la costra se rompe en placas cada vez más chicas que se hunden
  en lava cada vez más caliente, primero del lado del agujero; el planeta se
  encoge entero (sin deformarse) mientras pierde lo que se lleva el brazo, y
  encima hierve una corona delgada de vapor incandescente con llamas que
  suben (ruido estirado hacia afuera), abultada hacia el agujero. Es un
  volumen marchado en línea recta, de adelante hacia atrás: cada muestra
  emite con su color de cuerpo negro y tapa lo de atrás; la forma es un
  perfil suave erosionado por ruido. Cuando ya no queda planeta, lo último es
  un resplandor que solo brilla y se va con el brazo.
- **El brazo nace como un embudo** (`gargantua.rs::Stream`): sale de todo el
  lado de Pira que da al agujero (y se encoge con ella), casi derecho hacia
  el agujero, y después se curva y se adelgaza en un chorro. Lejos del
  agujero un paso del rayo mide más que el chorro, así que un paso largo se
  revisa contra una tabla del eje (dirección, altura, grosor y ancho cada
  tanto de radio) y, si pasa cerca, se suma en pedazos: sin eso se veían
  discos apilados.

- **Lunas** (`system.rs::Host`): una luna guarda una copia de la órbita de
  su planeta y su posición es la de él más la suya: sigue siendo función
  solo del tiempo. Si la órbita del planeta cambia (panel T), sus lunas se
  actualizan.
- **Vigía como su lámina**: paleta más profunda (índigo, morado, orquídea y
  rosa en las crestas), brazos en espiral logarítmica que caen al ojo y un
  iris de gas turbulento (ruido en polares que se retuerce y corre hacia
  adentro, con filamentos donde cruza por cero) que brilla rosa y magenta,
  con la pupila negra y redonda. Un ciclo lento le sube y le baja la fuerza a
  la tormenta y corre su color del rosa al violeta. Los relámpagos son
  destellos chicos dentro de las nubes del vórtice. Tiene **anillos
  delgados de polvo** que brillan a contraluz.
- **Los anillos de Vigía, como los de Saturno**
  ([shaders/rings.rs](src/shaders/rings.rs) `dust`): por radio, un anillo
  interior tenue y oscuro, uno ancho y denso con anillitos, la división
  oscura (como la de Cassini) y el exterior con un hueco angosto y otro más
  fino; cada zona con franjas de colores (lila, rosa, durazno, blanco hielo
  y gris azulado). Eso mismo da la sombra de los anillos sobre el planeta.
  Lo que depende del ángulo solo se dibuja: la lunita del hueco, que gira a
  su velocidad de Kepler y deja las orillas onduladas detrás (las olas se
  van calmando), un anillo de grumos por fuera que se trenza entre dos
  lunitas pastoras, y rayos radiales sobre el anillo denso que giran con el
  planeta y no con el anillo y aparecen y se desvanecen despacio. La sombra
  del planeta sobre los anillos ya estaba.
- **Vigía ya no es una pelota lisa** ([shaders/vigia.rs](src/shaders/vigia.rs)):
  - **Bandas** por latitud, cada una con su color y su corriente (al este o
    al oeste). La latitud se tuerce con un fbm y con ondas de dos armónicos
    a lo largo de la longitud (Kelvin-Helmholtz, como entre las bandas de
    Saturno), y adentro de cada banda hay remolinos chicos cuando se
    alcanzan a ver. Se doblan alrededor del ojo con menos giro que el
    detalle de las nubes (con todo el giro el disco parecía una paleta de
    dulce) y no se calculan debajo del iris, donde no se ven.
  - **Tormentas eléctricas**: cinco cúmulos de nubes claras y esponjadas en
    las bandas de latitudes medias. Adentro, rayos en celdas que destellan
    cada una a su hora y, a ratos, uno grande que alumbra toda la nube desde
    adentro; se ven de día y mucho más de noche. Cada pixel las descarta
    primero por latitud, que no cuesta.
  - **Auroras**: una cinta que serpentea alrededor de cada polo, con
    pliegues finos como cortinas, verde abajo y magenta arriba; laten con
    la estrella y se ven sobre todo de noche y en el borde.
  - **Relámpagos por todo el planeta**: celdas sobre la esfera que destellan
    cada una a su hora, sobre todo en el lado de noche.
- **Párpado y El espejo** ([shaders/parpado.rs](src/shaders/parpado.rs)): la
  luna de Vigía, acoplada por marea (le da siempre la misma cara), con
  cordilleras de cristal y lagos. Del lado de Vigía hay un lago grande: su
  agua refleja el cielo y, si el rayo reflejado pega en la esfera de Vigía,
  ese punto se sombrea **con el mismo shader de Vigía**, así que el ojo
  también se ve en el lago. Fresnel de Schlick; la nave levanta ondas
  (anillos que se alejan del punto bajo ella). Los racimos de cristal reflejan
  lo mismo y tienen vetas de luz que se encienden con la nave cerca. Es el
  lugar de Vigía para la tecla N.
- **Anillos de varios estilos** (`rings.rs::RingStyle`): el mismo disco
  analítico sirve para las naves de Cementerio, el polvo de Vigía (bandas
  delgadas con divisiones nítidas) y las ruinas de Réquiem (bloques y
  puentes: cajas giradas en las mismas celdas polares). Si un rayo cruza
  varios, se mezclan del más lejano al más cercano.
- **Réquiem como su lámina**: las agujas góticas se volvieron **monolitos**:
  losas de piedra oscura en tramos que no calzan, gastadas (vetas,
  grietas, escarcha que chorrea), con un glifo tallado en sus caras anchas
  (un círculo cruzado por una línea) que brilla apenas, late con la estrella,
  se enciende con la nave cerca y queda encendido después del colapso. En
  su lugar, todos miran a la estrella y varios se inclinan hacia ella:
  apuntan a donde va a nacer Gargantúa. Hay **picos de roca** oscuros en las
  alturas nevadas, **dunas** en la nieve (ondas de diente de sierra suave que
  inclinan la normal, solo de cerca), un **anillo de ruinas** y una luna,
  **Epitafio**, grande en su cielo.

# Cambios de la cuarta revisión

- **Las fases como las referencias**: la nebulosa del estertor tiene dos
  capas (filamentos y nubes) y se apaga con la estrella, porque era ella la
  que la iluminaba; en el silencio la exposición sube (los ojos se
  acostumbran) y aparece el cielo; el disco nace blanco azulado (15 800 K) y
  se enfría a naranja fuego (4 300 K).
- **Gargantúa se come a Pira** (`cycle::swallow`,
  `shaders/gargantua.rs::Stream`): seis segundos después de abrirse el ojo,
  los dibujos de Pira se deformaban con una matriz que la estiraba hacia el
  agujero (reemplazado en la quinta revisión); su corteza se funde y pierde
  su atmósfera y lo que tenía encima. A los 16 s ya no existe
  (`Body::hidden`: no se dibuja ni se choca con ella). Su gas entra al disco
  como un brazo en espiral **dentro del mismo volumen del Proyecto 2**: se
  curva con la lente igual que el disco y tiene su misma turbulencia (la
  textura de gas), así que al ensancharse y enrollarse (el gas de adentro
  gira más rápido) se confunde con él. Se calienta al caer, del rojo al
  blanco; cuando ya no hay Pira se vacía de afuera hacia adentro.
- **Morir en el horizonte** (`App::watch_horizon`): la nave guarda dónde
  estuvo cada medio segundo. A menos de 1.6 rs del agujero la pantalla se
  funde a negro y el tiempo vuelve 15 s, con la nave donde estaba.

# Cambios de la tercera revisión

- **Jardín** volvió a como estaba en la primera revisión.
- **La cola de Pira** ([shaders/tail.rs](src/shaders/tail.rs)): ya no hay
  un chorro que cae a la estrella. Un planeta que se desintegra deja una cola
  de polvo del lado contrario: la radiación lo empuja hacia afuera y el
  planeta, que sigue en su órbita, lo deja atrás. En el marco de la cola su
  eje es la parábola `y = c x²`; se marcha el rayo con 6 muestras, solo en
  el tramo entre el planeta y la punta y solo si pasa cerca del eje. El
  polvo dispersa la luz de la estrella hacia adelante (Henyey-Greenstein) y
  junto al planeta el vapor brilla con su color de cuerpo negro.
- **Lugares que reaccionan a la nave** (`System::sites`, `scene/props.rs`):
  cada planeta tiene un lugar fijo en su espacio, como el claro de Jardín.
  Los shaders reciben la posición de la nave (`Uniforms::ship`) y cada objeto
  despierta según su distancia: los géiseres hacen erupción (fuentes de
  partículas con gravedad), las agujas encienden sus ventanas desde lo más
  cercano, los naufragios recuperan algunas luces y echan chispas, la baliza
  titila más rápido y el ojo de Vigía cierra la pupila
  (`Uniforms::eye_near`). Cerca del suelo de cada planeta flotan brasas,
  gotas, polvo o destellos en un cubo que acompaña a la cámara (en Vigía ya
  no: los jirones de nube se veían como pedazos de cosas volando). Las luces de los
  géiseres, las agujas y la baliza se suman a las de los hongos (las 12 más
  cercanas). N viaja de un lugar al siguiente.

# Cambios de la segunda revisión

- **El final a pedido** (`scene/cycle.rs`): ya no hay un ciclo fijo de 15
  minutos. La estrella agoniza hasta que se presiona J; desde ahí, 20 s de
  estertor, 6 s de colapso, 4 s de silencio y el ojo se abre. Todo sigue
  siendo función pura del tiempo y de los instantes de J (`Timeline`), así
  que rebobinar funciona igual. Otra J hace renacer a la estrella con un
  fundido a blanco.
- **Estrella con un ojo** (`shaders/sun.rs`): una tormenta en un lugar fijo
  de la superficie (`EYE_AXIS`, en el espacio de la estrella): gira con ella,
  así que de un lado se ve de frente, de otro queda en el borde y desde atrás
  no se ve. A los 40 s de la agonía mira de frente a la vista de detrás de
  Pira, cerca de donde arranca la nave. Alrededor del ojo el punto se gira
  con la fórmula de Rodrigues (como el vórtice de Vigía) y todo el patrón da
  vueltas en bloque, así no se enrosca sin fin. El fuego es ruido con dos
  capas de *domain warping* que hierve con el tiempo: humo de detalle fino
  sobre regiones grandes que arden o se apagan, motas brillantes y lenguas
  finas donde arde, y velos malva en lo frío. Alrededor de la pupila hay un
  anillo de humo oscuro y después uno de fuego, como un iris. El calor se
  pinta con una rampa de brasa (negro, granate, carmesí, naranja, amarillo
  blanco); el borde es lo más caliente. La pupila se dilata con el latido y
  cada latido sale de ella como un anillo de luz. Si la estrella se calienta
  al colapsar, el fuego pasa a color de cuerpo negro.
- **Llamas y corona** (`shaders/sky.rs::corona`): `sun::limb_fire` da cuánto
  llega el fuego en cada ángulo del borde; el disco la evalúa en el limbo y
  el cielo por fuera, con el mismo ángulo, así que las llamas siguen de un
  lado al otro del borde. Alrededor, nubes de gas carmesí y magenta: son de
  baja frecuencia, así que se tabulan una vez por frame en una tabla polar
  (640 ángulos por 80 anillos) y cada pixel del cielo solo interpola. Eso
  bajó el costo extra de la estrella llenando la pantalla de 21 a 9 ms.
- **Arcos de plasma y chispas** (`scene/effects.rs`): cinco arcos grandes,
  tres arriba. Cada uno es un manojo de nueve hebras que se enroscan a
  distintos radios (se ve deshilachado, como fuego), encendido en los pies y
  rojo arriba, con nudos que bajan. Se anclan al borde tal como se ve desde
  la cámara, así que siempre se recortan contra el cielo; en el estertor
  crecen y uno de cada tres se desprende. Chispas salen despedidas del borde
  y se apagan.
- **Nebulosa** (`shaders/nebula.rs`): la capa de gas del estertor. Ver la
  sección 10.
- **Pira**: corteza en placas de Voronoi con lava entre ellas, más del lado
  que mira a la estrella; las grietas laten con retraso.
- **Vigía**: paleta morada propia sobre el detalle del Hubble, vórtice que
  gira, iris con fibras, pupila que se dilata con el latido y relámpagos.
- **Réquiem**: agujas góticas en vez de monolitos (pisos octogonales,
  cornisas con escarcha, ventanas de luz fría que sube con cada latido) y un
  mundo de hielo bajo una luz desaturada (la estrella queda lejos).
- **Cementerio**: los anillos están llenos de naves (siluetas por distancia
  con signo en celdas de Voronoi polares, con luces de navegación que
  parpadean) y una baliza de celosía emite una onda con cada destello.
- **Rendimiento**: mapa de sombras en paralelo por franjas sin caras de
  espaldas a la luz, mapas de roca empaquetados en un texel de 8 bytes, gas
  de Gargantúa cada dos pixeles con tabla radial, y FTZ/DAZ en todos los
  hilos. Las vistas que no son de Gargantúa salieron idénticas bit a bit.

# Cambios de la primera revisión visual

El resto de este documento describe la arquitectura original. Esa revisión
agregó los siguientes puntos para explicar en la demostración:

- **Frenado** (`scene/ship.rs`): antes la aceleracion maxima dependia solo de la
  altitud. El piloto podia llegar con mucha velocidad y quedarse sin fuerza
  para detenerse. Ahora el exceso de velocidad aporta capacidad de frenado;
  soltar las teclas usa ganancia 8/s. X pide detenerse con prioridad y compensa
  la gravedad incluso sin asistente. En modo local se frena contra la velocidad
  de la superficie, no contra el origen del universo.
- **Limite exterior**: empieza 1.200 unidades antes del borde, reduce solo la
  componente saliente de velocidad y permite regresar. El borde se obtiene de
  la orbita mas lejana mas un margen; sigue funcionando al editar los radios.
- **Normales triplanares** (`shaders/detail.rs`): tres proyecciones de mapas
  fotográficos y procedurales se mezclan según la normal. Las pendientes pasan del espacio del
  modelo al mundo con su matriz de normales. Por eso el relieve gira con el
  planeta. Los mipmaps evitan ruido fino a distancia.
- **Nave**: tres OBJ de la OCASO-1 adjunta, con UV, normales y parámetros
  originales de los materiales. Se selecciona un nivel según la
  distancia a la camara. El modelo Blender original se conserva sin cambios.
- **Hongos y torres**: las nervaduras, arcos y huecos son triangulos reales
  generados antes de rasterizar. La distribucion usa la semilla del sistema.
- **Vigía**: usa el mapa fotográfico Hubble de NASA con una paleta violeta.
  El shader deforma sus coordenadas alrededor de una tormenta que sigue a la
  nave. La vegetación de Jardín se desvanece progresivamente a gran altitud
  para conservar la lectura del planeta desde órbita.
- **FXAA** (`render/antialias.rs`): identifica diagonales por luminancia y
  combina muestras bilineales del framebuffer RGBA. Funciona en CPU; la GPU
  sigue recibiendo solamente la imagen 2D final.
- **Recorrido**: `--recorrido` usa el mismo integrador de 120 Hz y los mismos
  controles. El piloto se acerca, frena, rodea Jardin, se aleja y vuelve a
  frenar. La exportacion corre a simulacion x3 y no representa FPS en vivo.

Los parametros actuales estan en `config.rs`, `scene/system.rs` y
`scene/cycle.rs`; prevalecen sobre cifras de la explicacion original.

---

# Cómo funciona OCASO

Explicación de cada parte del proyecto, en el orden en que pasa un frame.
Cada sección dice qué archivo mirar. La idea es poder responder "¿cómo está
hecho?" para cualquier característica del menú.

## 1. Un frame de principio a fin

1. **Entrada** ([main.rs](src/main.rs)): raylib lee teclado y mouse y los
   traduce a un `FrameInput` (empuje, mira, rueda, rebobinar).
2. **Simulación** ([app.rs](src/app.rs), `App::update`): la mira sigue al
   mouse cada frame; la física avanza en pasos fijos de 1/120 s.
3. **Armado del frame** (`App::render`): se calcula el estado del ciclo, la
   cámara, qué chunks de terreno y qué objetos se ven, las partículas, la
   trayectoria y Gargantúa. Todo termina en una lista de `Draw` (malla +
   matriz + material) y un `Uniforms` (lo común a todos los pixeles).
4. **Render** ([render/mod.rs](src/render/mod.rs)): el pipeline por software
   produce una imagen RGBA8.
5. **Presentación**: raylib solo copia esa imagen a una textura, la dibuja y
   pone el HUD de texto encima.

Ninguna función 3D de raylib se usa: ni cámara, ni modelos, ni shaders.

## 2. El pipeline por software

### Transformaciones
- **Modelo**: cada `Draw` trae una matriz que lleva la malla al mundo
  (rotación, escala y traslación).
- **Relativo a la cámara**: las posiciones del mundo se guardan en `f64`
  (el sistema mide decenas de miles de unidades) y antes de pasar a `f32` se
  les resta la posición de la cámara. Así cerca de la cámara hay precisión
  de sobra y nada tiembla.
- **Vista y proyección** ([math/mat.rs](src/math/mat.rs)): la vista solo
  rota (la cámara ya está en el origen). La proyección es de **profundidad
  invertida con plano lejano en el infinito**: `z_ndc = near / w`, que vale
  1 en el plano cercano y tiende a 0 lejos. Con `f32`, esa forma reparte la
  precisión de manera casi uniforme en escala logarítmica: se puede ver un
  planeta a 6000 unidades y la nave a 2 sin peleas de profundidad.

### Triángulos ([render/pipeline.rs](src/render/pipeline.rs))
- **Descarte de caras traseras**: las mallas tienen sus vértices en sentido
  antihorario vistos desde afuera; en pantalla, el signo del área del
  triángulo dice si se ve de frente o de espaldas, y los de espaldas se
  descartan.
- **Recorte**: Sutherland–Hodgman contra el plano cercano (un triángulo
  cortado puede volverse un polígono de hasta 4 o 5 vértices, que se abre
  en abanico). Contra los lados solo se recorta si un vértice sale 8 veces
  del frustum (banda de guarda): lo demás lo limita el raster.
- **Setup**: coordenadas de pantalla en punto fijo (8 bits de subpixel) y las
  ecuaciones de arista en enteros de 64 bits, con la regla *top-left*: dos
  triángulos que comparten una arista nunca pintan el mismo pixel ni dejan
  huecos.

### Raster y shading por tiles ([render/raster.rs](src/render/raster.rs), [render/shade.rs](src/render/shade.rs))
- La pantalla se parte en tiles de 64×64. Cada triángulo se anota en los
  tiles que toca y cada tile lo procesa un solo hilo, sin candados.
- **Visibility buffer**: el raster no corre shaders; guarda por pixel la
  profundidad y el id del triángulo más cercano.
- **Shading diferido**: después, cada pixel visible reconstruye sus
  atributos (posición, normal, uv, color, tangente) con baricéntricas
  corregidas por perspectiva (`atributo / w` es lineal en pantalla) y corre
  el shader de su material **una sola vez**. Con shaders caros eso ahorra
  todo el overdraw.
- En la misma pasada por tile se suman las partículas y se extrae el brillo
  para el bloom, mientras el tile sigue en caché.

### Post ([render/bloom.rs](src/render/bloom.rs), [render/post.rs](src/render/post.rs))
- **Bloom**: una pirámide de 6 niveles con blur gaussiano separable.
- **Tonemap**: curva ACES aplicada al canal máximo (conserva el tono: el
  rojo de la estrella no se lava a blanco), conversión a sRGB con una tabla
  de 4096 entradas y un poco de dither.

### Paralelismo ([parallel.rs](src/parallel.rs))
Un pool de hilos persistentes con reparto dinámico: cada hilo toma el
siguiente trabajo de un contador atómico. El i5-12500H tiene núcleos rápidos
y lentos; con reparto fijo, los rápidos esperarían a los lentos.

## 3. Los planetas

### Órbitas y giros ([scene/system.rs](src/scene/system.rs))
Todo movimiento es una **función del tiempo**: `angulo = fase + 2π t / periodo`.
No se acumula nada frame a frame, así que no depende de los fps y rebobinar
es evaluar con un `t` menor. Pira tiene acoplamiento de marea: gira con el
mismo periodo y la misma fase que su órbita y siempre le muestra la misma
cara a la estrella.

### Terreno de cubo-esfera con LOD ([terrain/](src/terrain))
- Cada cara de un cubo se proyecta sobre la esfera (*spherified cube*) y es
  la raíz de un **quadtree**. Cada nodo es un chunk de 32×32 cuadros.
- **Deformación con ruido**: cada vértice se desplaza sobre la normal de la
  esfera según la función de altura del planeta
  ([terrain/shapes.rs](src/terrain/shapes.rs)): ruido fBm, crestas
  (*ridged*), cráteres con ruido celular, mares de lava, zanjas.
- **LOD**: un nodo se parte en cuatro si la cámara está más cerca que 1.8
  veces su tamaño. Así cada chunk ocupa más o menos lo mismo en pantalla.
- **Culling**: un nodo detrás del horizonte (prueba
  `acos(R/d) + acos(R/ρ)`) o fuera del frustum se descarta con todo su
  subárbol.
- **Grietas**: dos chunks vecinos de distinto nivel no comparten todos sus
  vértices; cada chunk lleva un faldón que baja hacia el centro y tapa la
  grieta.
- Los chunks nuevos se generan en paralelo, hasta 24 por frame, los más
  cercanos primero; mientras falta un hijo se dibuja el padre.

### Las identidades
| Planeta | Rasgo central |
| --- | --- |
| Pira | Corteza negra en placas con lava de cuerpo negro entre ellas, grietas que laten, acoplamiento de marea, una cola de polvo que la estrella le arranca y un campo de géiseres. |
| Jardín | Arterias que laten con la estrella, poros, capilares; la superficie respira (desplazamiento en el vertex shader). |
| Vigía | Gigante gaseoso morado con un vórtice que gira y una tormenta en forma de ojo, de gas rosa, que sigue a la nave; la pupila late. Anillos delgados de polvo. |
| Párpado | La luna de Vigía: cristal y lagos espejo donde se refleja el ojo. |
| Argos | Carne en pliegues con un ojo enorme que gira dentro de ella para seguir a la nave por todo el sistema, con una corona de ojos chicos en sus párpados; parpadea, cierra la pupila de cerca y, volando bajo, va debajo de la nave. La carne está llena de ojos chicos que también te miran. |
| Faro | La compañera muerta: una estrella de neutrones de 12 km, con sus proporciones reales, vista a través de su propia gravedad, con su magnetosfera torcida y dos haces que barren el sistema (ver la primera sección). |

## 4. Shaders ([shaders/](src/shaders))

Cada material es una función que recibe los atributos del pixel y devuelve
su color HDR. Algunos ejemplos de técnica:
- **Estrella** ([sun.rs](src/shaders/sun.rs)): un ojo fijo en la
  superficie con un remolino de Rodrigues, fuego con dos capas de *domain
  warping*, una pupila que late, el borde encendido con las mismas llamas que
  siguen en la corona del cielo y las llamaradas del estertor.
- **Lava** ([pira.rs](src/shaders/pira.rs)): placas de Voronoi; el color de la
  lava sale de la ley de Planck (temperatura → color de cuerpo negro). Las
  grietas más delgadas que un pixel se ensanchan y bajan su brillo en la
  misma proporción.
- **Jardín** ([jardin.rs](src/shaders/jardin.rs)): las arterias son las
  líneas donde un ruido cruza un valor; cuando un pixel cubre más que la
  línea, se dibuja su brillo promedio en vez de parpadear.
- **Argos** ([argos.rs](src/shaders/argos.rs)): el ojo se dibuja en
  polares alrededor de su eje, que cambia cada frame; la normal del globo y
  la pendiente de la córnea y los párpados salen de la misma altura que usa
  el vertex shader, derivada con diferencias finitas. Cada región (carne,
  esclera, iris) calcula su ruido solo donde se ve.
- **Vigía** ([vigia.rs](src/shaders/vigia.rs)): coordenadas giradas con la
  fórmula de Rodrigues para el vórtice; el iris en coordenadas polares
  alrededor del eje del ojo, con ruido que se retuerce y corre hacia adentro.
  El lado de noche no se apaga: las capas hondas del gas brillan violeta a
  través de las nubes, el iris alumbra el remolino con una caída gaussiana
  y, a contraluz, un borde de dispersión hacia adelante cierra la silueta.
  Sin eso, desde Párpado o desde atrás se veía medio planeta o el ojo solo.
- **Párpado** ([parpado.rs](src/shaders/parpado.rs)): el agua refleja a
  Vigía intersectando el rayo reflejado con su esfera y llamando al shader de
  Vigía en ese punto.
- **Faro** ([faro.rs](src/shaders/faro.rs)): la esfera que se rasteriza es
  solo el disco aparente; cada pixel busca en la tabla de geodésicas de qué
  punto de la estrella salió su rayo y pinta ahí la corteza agrietada con la
  temperatura de ese punto (Greenstein-Hartke y casquetes). Sus haces son
  un volumen: el rayo se intersecta con el
  doble cono de forma analítica y, adentro, se integra el perfil de Rankin
  con el cambio de variable que vuelve exacto el `1/r²`.
- **Anillos** ([rings.rs](src/shaders/rings.rs)): analíticos (intersección
  rayo-plano), con la sombra del planeta sobre ellos y la de ellos sobre el
  planeta. La sombra del planeta tiene penumbra: su ancho es la distancia
  detrás del planeta por el radio angular de la estrella.

**Animados**: todos los que usan `u.time` o el latido: estrella, lava, venas,
vórtice, tormenta y pupila de Vigía, ojo de Argos, cristales, lagos, anillos,
nebulosa, Pira hirviendo, gas del disco y Faro (los haces barren con el giro,
sus chispas dan vueltas, cada pulso es distinto y los casquetes se calientan
con los estallidos y la llamarada).

## 5. Mapeo de normales ([shaders/textures.rs](src/shaders/textures.rs))

- Los mapas se generan al arrancar a partir de un **mapa de alturas** dibujado
  con funciones de distancia (las placas del casco). La
  normal de cada texel sale de la pendiente:
  `n = normalize(-dh/du, -dh/dv, 1)`.
- Cada vértice lleva una **tangente** (hacia donde crece `u`) y un signo.
  En el pixel se rearma la base **TBN**: la tangente se vuelve a hacer
  perpendicular a la normal (Gram-Schmidt) y la bitangente es `n × t`. La
  normal del mapa se lleva al mundo con esa base (`Fragment::perturb`).
- **Mipmaps**: cada nivel promedia el anterior y se lee el nivel donde un
  texel mide lo mismo que el pixel. De lejos las normales de un surco se
  cancelan y la piedra se ve lisa, sin parpadeo.

## 6. Objetos de la superficie ([scene/props.rs](src/scene/props.rs))

- **Modelos**: siete tipos en OCASO (y trece de la versión chapina, en
  [chapin/](src/chapin)) armados con código con `MeshBuilder`
  ([scene/builder.rs](src/scene/builder.rs)): cajas, conos truncados,
  elipsoides y tubos con transporte paralelo. Cada uno en tres niveles de
  detalle.
- **Distribución reproducible**: cada cara del cubo se divide en una grilla;
  cada celda propone un candidato en un punto pseudoaleatorio (un hash de la
  semilla, la cara y la celda). Se acepta con una probabilidad que sale de
  **ruido fBm** (grupos y claros) por las reglas del terreno (no hay hongos en
  la lava ni ojos en el fondo de los surcos). La misma semilla da las mismas
  posiciones; F6 la cambia.
- **Posición y orientación**: cada objeto se guarda en el espacio de su
  planeta, con +Y sobre la normal radial y un giro al azar alrededor de ella;
  se hunde un poco para no flotar en las pendientes. Al dibujarlo, su matriz
  es la del planeta por la suya: gira con el planeta. En Jardín sube y baja
  con la respiración.
- **LOD por tamaño en pantalla** (radio en pixeles de su esfera envolvente):
  los muy chicos no se dibujan y cerca de ese límite se encogen hacia el
  suelo en vez de desaparecer de golpe.
- **Sombra del planeta por pixel**: el rayo hacia la estrella pasa a una
  distancia `d` del centro; si `d` es menor que el radio, el planeta lo tapa.
  El sombrero de un hongo sigue iluminado cuando su base ya está de noche.

## 7. Navegación ([scene/ship.rs](src/scene/ship.rs))

- **Paso fijo**: la física avanza en pasos de 1/120 s sin importar los fps;
  el dibujo interpola entre los dos últimos pasos. La misma tecla el mismo
  tiempo da la misma trayectoria en cualquier computadora.
- **Aceleración progresiva**: con el asistente, las teclas piden una
  *velocidad*; los propulsores corrigen la diferencia con aceleración
  limitada. El giro persigue la mira con velocidad y aceleración angular
  limitadas. La velocidad máxima crece con la altitud.
- **Marco de referencia**: posición y velocidad se guardan relativas al
  cuerpo dominante (el de región más pequeña que contiene a la nave). Cerca
  de un planeta la nave viaja con él.
- **Gravedad**: `a = -μ r / |r|³` del cuerpo dominante, integrada con Euler
  semi-implícito (conserva bien la energía de una órbita). Sin asistente la
  nave es newtoniana: soltar las teclas no la detiene. Los propulsores
  empujan con la mitad de la aceleración del asistente (siempre más que la
  gravedad, para poder despegar) y dejan de empujar hacia adelante al llegar
  a la velocidad máxima; la gravedad sí puede pasarla. Antes usaban la
  aceleración del asistente, que crece con el exceso de velocidad para
  frenar a tiempo: ir rápido empujaba más y en tres segundos de W la nave
  pasaba de 1400 u/s.
- **Predicción de trayectoria** ([scene/trajectory.rs](src/scene/trajectory.rs)):
  se integra la caída libre 90 s hacia adelante con la misma física y se
  dibuja con cintas; si choca, se marca el punto.
- **Modo local** (L): la velocidad que se pide es relativa a la superficie
  que gira (`ω × r`); la mira se nivela con la vertical del planeta y se
  transporta con la curvatura (el giro mínimo que lleva la vertical anterior
  a la nueva), así avanzar derecho rodea el planeta por un círculo máximo; la
  altura sobre el terreno se sostiene sola mirando el suelo que viene en un
  cuarto, medio y un segundo (a la velocidad pedida y a la real), y para no
  raspar el terreno puede subir más rápido que el límite. Mirando hacia
  arriba o hacia abajo más de unos 11 grados, W sube o baja por donde se
  mira; bajando, solo tan rápido como para frenar antes del suelo. Antes W
  solo movía a la misma altura y la nave se sentía atascada. Si cambia el
  planeta de abajo (de un planeta a su luna), la mira no se transporta, solo
  se nivela. El crucero (O) empuja hacia adelante solo.
- **Cuerpos ocultos**: no cuentan como marco ni como suelo. Antes, en
  OCASO, la Luna de Xelajú (que solo existe en la versión chapina) seguía
  en la física: cerca de Argos la nave se enganchaba a una luna invisible y
  chocaba con su terreno.
- **Colisión**: contra la misma función de altura que se dibuja; se empuja la
  nave afuera y se quita la velocidad hacia adentro con un rebote pequeño.

## 8. Rendimiento: descarte y niveles de detalle

| Qué | Dónde | Cómo se ve |
| --- | --- | --- |
| Esfera envolvente de cada dibujo contra los 5 planos útiles del frustum | `Renderer::render` | HUD: "objetos, fuera de vista" |
| Chunks detrás del horizonte o fuera del frustum | `Terrain::select` | HUD: "horizonte -N frustum -N" |
| Objetos demasiado chicos, detrás del horizonte o fuera del frustum | `props::select` | HUD: "objetos" |
| Caras traseras y triángulos sin pixel | `pipeline.rs` | HUD: "espalda -N, sin pixel -N" |
| LOD del terreno y de los objetos | quadtree, `props::select` | F4 colorea niveles; F5 apaga el LOD y el contador de triángulos sube |
| Frustum congelado | F7 | Se vuela por fuera y se ve qué queda dentro |

## 9. Atmósferas ([shaders/atmosphere.rs](src/shaders/atmosphere.rs))

Dispersión simple de Rayleigh (más en el azul) y Mie (hacia adelante, el halo
del sol). La profundidad óptica hacia la estrella sale de una tabla de
32×128 (altura × ángulo) con la sombra del planeta. Sobre las superficies la
atmósfera se integra por vértice (perspectiva aérea) y se interpola; el cielo,
por pixel, o desde una tabla del cielo de 64×96 cuando la cámara está dentro
de la atmósfera.

## 10. El ciclo y Gargantúa

### El ciclo ([scene/cycle.rs](src/scene/cycle.rs))
La agonía dura lo que el piloto quiera; con J: estertor (el latido acelera de
1.9 a 0.7 s, la estrella se hincha y suelta su capa de gas), colapso en caída
libre (`R ∝ (1 - u²)^(2/3)`) mientras la luz se drena, silencio, el ojo se
abre (onda gravitacional y destello) y el disco se enciende. Todo es función
pura del tiempo y de los instantes en que se presionó J. Como el latido
acelera, su fase no puede ser `t / periodo` (saltaría al cambiar el
periodo): es la integral cerrada de la frecuencia.

### La nebulosa ([shaders/nebula.rs](src/shaders/nebula.rs))
No se marcha un volumen: cada rayo corta la esfera de la capa en a lo más
dos puntos y ahí se evalúan los filamentos (donde un ruido cruza por cero,
deformado por otro ruido) y unos grumos que dejan regiones vacías. La luz es
la del tramo que el rayo pasa dentro de la cáscara: su grosor entre el
coseno con la normal, así el borde brilla más, como en las nebulosas reales.

### Gargantúa (`shaders/gargantua.rs`, `gargantua_p2/`)

- Se integra el núcleo de la Variante predeterminada del Proyecto 2,
  conservado bajo `gargantua_p2/` para poder comparar sus fórmulas y pruebas.
  No se modifica el Proyecto 2 ni se depende de su ejecutable.
- Unidades `rs = 1`, masa `M = 1/2`, geodésica
  `a = -(3/2) h² r / |r|⁵`, RK4 con pasos adaptativos. La dirección local
  se lleva al marco de Schwarzschild antes de integrar. La base del disco
  mantiene la orientación para conservar el signo del Doppler.
- Espesor gaussiano: fotosfera de `0.004 r`, atmósfera de `0.028 r`,
  disco caliente de 3 a 13 rs y envoltura gris hasta 19 rs. Cada tramo
  aporta emisión y absorción de adelante hacia atrás. La opacidad y la
  fuente se enfrían gradualmente en la periferia.
- Pico de color 6500 K, corrimiento gravitacional y Doppler, brillo `g⁴`
  aplicado una sola vez. El tono dorado conserva la compresión de rango
  del Proyecto 2 y exposición 5.0. El resto del ciclo conserva su ACES.
- Gas 3D de 384×192×9, dos campos que se relevan continuamente, rotación
  de Kepler y cizalla con altura. No se sustituye por anillos de seno.
- La tabla angular sigue acelerando el cielo que no atraviesa gas.
- El gas se traza cada dos pixeles; un pixel entre cuatro muestras
  parecidas las interpola, y si no se parecen (la sombra, la orilla del
  disco, el anillo de fotones) traza su propio rayo. Un rayo que sale de la
  envoltura ya no vuelve a cruzar gas y se corta. Lo que depende solo del
  radio (temperatura, Kepler, opacidad) y el color de cuerpo negro van en
  tablas; `gargantua_p2/disk.rs::prepare` sigue intacto como referencia.
  Superficies delanteras conservan su visibilidad; la sombra del agujero
  aporta profundidad para tapar anillos y atmósferas posteriores.

### Luz y materiales (`shaders/pbr.rs`, `render/shadow.rs`)

Cook–Torrance con distribución GGX, geometría Smith y Fresnel Schlick.
Rugosidad y metalicidad provienen del Blender en la nave; fotografías y
mapas triplanares aportan detalle a los planetas. Un mapa de sombras
ortográfico 1024² resuelve sombras cercanas con PCF de nueve muestras: se
rasteriza en paralelo por franjas de 16 filas y sin las caras de espaldas a
la luz (en una superficie cerrada nunca son lo más cercano a ella). El raster completo termina antes del shading para
poder leer vecinos del z-buffer en la oclusión ambiental. Un segundo
buffer evita leer y escribir simultáneamente los reflejos de pantalla.
Estos reflejos solo alcanzan superficies cercanas visibles en pantalla.
Las bioluminiscencias emiten y aportan luces puntuales de alcance finito.

## 11. Partículas, sonido y ajustes en vivo

- **Partículas** ([render/particles.rs](src/render/particles.rs),
  [scene/effects.rs](src/scene/effects.rs)): son luz, no superficie. Cada una
  se proyecta y se **suma** como una mancha gaussiana solo donde queda
  delante de la superficie. Cada partícula es una función del tiempo (su edad
  es `fract(t / vida + fase)`), sin estado.
- **Sonido** ([audio.rs](src/audio.rs)): se sintetiza muestra por muestra: el
  latido lub-dub (un seno que baja de 70 a 38 Hz con un chasquido), motor,
  viento y un acorde de fondo. raylib solo lo reproduce.
- **Ajustes en vivo** ([tweaks.rs](src/tweaks.rs), [scene/tuning.rs](src/scene/tuning.rs)):

| Si se cambia... | Qué pasa y por qué |
| --- | --- |
| El radio de un planeta | Se crea un terreno nuevo con ese radio (los chunks se regeneran alrededor de la cámara), se recalcula la tabla de su atmósfera y se vuelven a repartir sus objetos. La gravedad en la superficie se conserva: `μ = g r²`. |
| Su periodo orbital | Cambia la velocidad en la órbita sin que salte: la fase se corrige para que en este instante siga en el mismo ángulo. Si estaba acoplado por marea (Pira), su giro cambia con él. |
| Su periodo de giro | Igual, con la fase del giro. |
| La intensidad de su shader | Escala su efecto propio (la lava, la bioluminiscencia, el ojo y los cristales, el iris de Argos y las luces de la capital). |
| La semilla | Otras posiciones para todos los objetos, siempre las mismas para esa semilla. |
| El factor de LOD | Más o menos chunks (se ve en el contador de triángulos). |
