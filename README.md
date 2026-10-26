# OCASO

Gráficas por Computadora · Proyecto 3 · Lázaro Díaz

Un sistema solar explorable en nave, dibujado con un renderer por software
propio en CPU. Una supergigante roja agoniza y late como un corazón; al final
de cada ciclo colapsa y nace un agujero negro. La experiencia completa está en
desarrollo: este README describe lo que ya funciona.

## Estado: fase 0, el motor base

- Pipeline propio en CPU con matrices de modelo, vista y proyección,
  recorte contra el plano cercano, descarte de caras traseras y prueba de
  profundidad.
- Raster por tiles de 64×64 en paralelo y visibility buffer con shading
  diferido: cada pixel visible ejecuta su fragment shader una sola vez.
- Profundidad invertida en `f32` con plano lejano en el infinito, y render
  relativo a la cámara con posiciones del mundo en `f64`.
- Una estrella con shader animado (convección, limbo y latido) y un planeta
  que orbita y rota en función del tiempo, con terreno pintado por ruido.
- La nave modelada en Blender (la del lab 4), con física a paso fijo,
  asistente de vuelo, marco de referencia del planeta, gravedad y colisión.
- Cielo procedural, tonemap HDR y un HUD con contadores de triángulos por etapa.

## Ejecutar

Se necesita Rust (edición 2024) y CMake, que usa `raylib` para compilarse.

```sh
cargo run --release
```

Opciones:

```sh
cargo run --release -- --res 1920x1080        # resolución del render
cargo run --release -- --captura capturas     # vistas fijas en PNG, sin ventana
cargo run --release -- --bench 300            # mide el render en un recorrido fijo
cargo test --release                          # pruebas
```

## Controles

| Acción | Control |
| --- | --- |
| Apuntar (la nave gira hacia la mira) | Mouse |
| Mirar alrededor sin girar la nave | Clic derecho sostenido |
| Adelante / atrás | W / S |
| Lados | A / D |
| Subir / bajar | Espacio / Ctrl |
| Alabeo | Q / E |
| Turbo | Shift |
| Acercar / alejar la cámara | Rueda |
| Cámara de persecución / cabina | C |
| Asistente de vuelo | Z |
| Gravedad | G |
| Foto en `capturas/` | P |
| Soltar o capturar el mouse | Tab |
| Ayuda / estadísticas | F1 / F2 |
| Salir | Esc |

## Cómo funciona

```text
dibujos (malla + matriz + material)
 -> culling por objeto: esfera envolvente contra el frustum
 -> vértices: objeto -> mundo relativo a la cámara -> clip         (paralelo)
 -> triángulos: descarte, recorte, setup y reparto en tiles        (paralelo)
 -> raster por tile: profundidad + id del triángulo visible        (paralelo)
 -> shading diferido: un fragment shader por pixel visible         (paralelo)
 -> tonemap HDR -> RGBA8 -> textura de raylib
```

| Carpeta | Contenido |
| --- | --- |
| [src/render](src/render) | Pipeline: vértices, recorte, raster en punto fijo, shading diferido, tonemap y PNG |
| [src/shaders](src/shaders) | Fragment shaders: estrella, planeta, nave y cielo |
| [src/scene](src/scene) | Lector OBJ/MTL, icosfera, cuerpos celestes, física de la nave y cámara |
| [src/math](src/math) | Vectores, matrices, cuaterniones, ruido de Perlin y color de cuerpo negro |
| [src/parallel.rs](src/parallel.rs) | Pool de hilos persistentes con reparto dinámico |

## Dependencias, recursos y asistencia

- La única dependencia es `raylib`, usada solo para la ventana, la entrada,
  mostrar el framebuffer como textura y el texto 2D del HUD. No se usa
  ninguna de sus funciones 3D, de modelos, cámara, materiales ni shaders.
- Reutilizo código propio: el pool de hilos, el color de cuerpo negro y la
  curva de tonemap vienen de mi Proyecto 2; el lector OBJ parte del lab 4.
- El modelo de la nave es mío (Blender).
- Se usó asistencia de IA durante el desarrollo.
