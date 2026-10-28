# OCASO

Gráficas por Computadora · Proyecto 3 · Lázaro Díaz, 24713

Un sistema solar explorable en nave, dibujado con un renderer por software
propio en CPU. **Ocaso**, una supergigante roja, agoniza y late como un
corazón, y se puede volar entre sus cinco planetas todo el tiempo que se
quiera. Con **J** empieza el final: el latido se acelera, la estrella suelta
una capa de gas que arde, colapsa en una supernova fallida y en su lugar nace
**Gargantúa**, un agujero negro con su disco de acreción que curva la luz de
todo el cielo. Gargantúa hace hervir a Pira hasta deshacerla y su gas entra
al disco en espiral hasta mezclarse con él. Se puede volar al disco; quien cruza el
horizonte muere y el tiempo se rebobina. Otra J hace renacer a la estrella.

![Gargantúa: el ojo se abre](media/24_el_ojo.png)

| | |
| --- | --- |
| ![Agonía: la estrella late y Pira se le deshace encima](media/22_agonia.png) | ![Estertor: la estrella suelta su capa de gas](media/25_estertor.png) |
| ![Vigía y su ojo-tormenta](media/06_vigia.png) | ![Pira: corteza en placas y lava](media/02_pira.png) |
| ![Argos: el ojo que te sigue por todo el sistema](media/07_argos.png) | ![Cementerio: naves en los anillos y la baliza](media/08_cementerio.png) |
| ![Superficie de Jardín](media/23_jardin_biopunk.png) | ![Pira y su cola de polvo](media/14_cola.png) |
| ![Gargantúa se come a Pira](media/30_comida.png) | ![El gas de Pira se mezcla con el disco](media/31_digestion.png) |
| ![El ojo de Vigía reflejado en un lago de Párpado](media/32_espejo.png) | ![El iris de Argos de cerca](media/33_iris.png) |

## Lugares para visitar

Cada planeta tiene un lugar en su superficie que **reacciona a la nave**,
con luces que alumbran lo de alrededor y algo que flota a su lado. La tecla
**N** viaja al siguiente; cerca de cada planeta, el HUD marca su lugar.

| | |
| --- | --- |
| ![Los géiseres de Pira](media/27_geiseres.png) | **Los géiseres** (Pira): bocas de lava que hacen erupción de vez en cuando y despiertan al pasar la nave; alumbran el suelo y suben brasas a su alrededor. |
| ![El nido de Argos](media/28_nido.png) | **El nido** (Argos): carne granate y mojada, en pliegues, llena de ojos que miran a la nave y parpadean cada uno a su ritmo. Al bajar a la carne, el ojo grande se cierra y se hunde; al subir, vuelve a salir debajo de la nave y la sigue. Flotan gotas rojas. |
| ![El espejo en Párpado](media/32_espejo.png) | **El espejo** (Párpado, la luna de Vigía): un lago quieto entre cristales, con Vigía del otro lado del agua. El ojo te mira y también se ve en el lago, porque el reflejo se sombrea con el mismo shader de Vigía. La nave levanta ondas al pasar sobre el agua y los cristales se encienden por dentro. |
| ![La baliza de Cementerio](media/26_baliza.png) | **La baliza** (Cementerio): naves caídas alrededor de la torre. A su paso les vuelven las luces y echan chispas; la baliza titila más rápido y su onda sube. |
| ![El ojo de Vigía de cerca](media/29_ojo_cerca.png) | **El ojo** (Vigía): sigue a la nave y, cuando se acerca, cierra la pupila, enciende el iris y los relámpagos se juntan debajo de ella. Jirones de nube pasan rozando. |
| ![El claro de Jardín](media/11_hongos.png) | **El claro** (Jardín): colonias de hongos y corales que alumbran el suelo, con esporas flotando. |

## Video

El repositorio incluye [un recorrido continuo](media/recorrido-continuo.mp4)
(vuelo, acercamiento, una vuelta completa a Jardín y alejamiento) grabado
antes del rediseño de los planetas: hay que regrabarlo con `--recorrido`. El
piloto usa la misma física del vuelo manual; se exporta a 24 fps con la
simulación acelerada x3, así que no es una medición de FPS en tiempo real.

Recorrido sugerido para grabarlo (F9 oculta el HUD):

1. Desde el inicio, junto a Pira, alejarse con Shift para ver la cola de
   polvo que la estrella le arranca.
2. Volar a Jardín y bajar hasta la superficie: el modo local se activa solo.
3. Con O (crucero), dar la vuelta al planeta sobre el bosque de hongos.
4. Subir y alejarse con Shift.
5. J: el estertor (la nebulosa y las llamaradas), el silencio, la onda
   gravitacional, Gargantúa naciendo blanca azulada y comiéndose a Pira.

## Ejecutar

Se necesita Rust (edición 2024) y CMake, que usa `raylib` para compilarse.

```sh
cargo run --release
```

Opciones:

```sh
cargo run --release -- --res 1920x1080        # resolución del render (por defecto 1280x720)
cargo run --release -- --captura capturas     # las 33 vistas fijas como PNG, sin abrir ventana
cargo run --release -- --bench 30             # mide cada vista fija y da medianas por etapa
cargo run --release -- --wire --lod           # empieza con wireframe y colores de LOD
cargo test --release                          # pruebas de vuelo, raster, sombras y gas volumétrico
```

## Controles

| Acción | Control |
| --- | --- |
| Apuntar (la nave gira hacia la mira) | Mouse |
| Mirar alrededor sin girar la nave | Clic derecho sostenido |
| Adelante / atrás, lados | W / S, A / D |
| Subir / bajar | Espacio / Ctrl |
| Alabeo | Q / E |
| Turbo | Shift |
| **Freno de emergencia** (prioridad sobre turbo y crucero) | **X sostenida** |
| **Recuperar posicion** junto a Pira, conservando el ciclo | **Inicio / Home** |
| **Viajar al siguiente lugar** (claro, géiseres, nido, baliza, espejo) | **N** |
| Acercar / alejar la cámara | Rueda |
| Cámara de persecución / cabina | C |
| Asistente de vuelo / gravedad | Z / G |
| **Modo local** (vuelo relativo a la superficie) | L |
| **Crucero**: rodea el planeta solo | O |
| Trayectoria predicha (auto / siempre / nunca) | V |
| Rebobinar el tiempo | R sostenido |
| Saltar 30 s atrás / adelante en el tiempo | [ / ] |
| **Desatar el final** de la estrella / hacerla renacer | **J** |
| Recorrer las vistas fijas (las mismas de `--captura`) | AvPág / RePág |
| **Ajustes en vivo** (radio, órbita, semilla, shaders...) | T, flechas y Shift |
| Wireframe / colores de LOD / LOD encendido | F3 / F4 / F5 |
| Otra semilla para los objetos de la superficie | F6 |
| Congelar el frustum (ver el culling desde afuera) | F7 |
| Silenciar | M |
| Foto en `capturas/` | P |
| Soltar o capturar el mouse | Tab |
| Ayuda / estadísticas / ocultar todo el HUD | F1 / F2 / F9 |
| Salir | Esc |


## Revision visual y vuelo

En Windows se puede abrir `Jugar_OCASO.cmd`; en el paquete lleva el ejecutable,
y en el repositorio compila con Cargo si no lo encuentra. Los controles completos
se muestran con F1; F2 abre contadores y F9 oculta el HUD.

- OCASO-1: 78.057, 24.977 y 6.243 triangulos segun distancia.
- Jardin: hongos con nervaduras y torres abiertas con nucleos luminosos;
  suelo con normales triplanares, cavidades y arterias animadas. El bioma
  de llegada tiene colonias a ambos lados de un claro, colocadas sobre el
  terreno; giran con el planeta y se pueden visitar en vuelo libre.
- Cielo con estrellas mas finas y nebulosa; estrella roja con conveccion;
  Gargantua con la Variante dorada y las nubes grises del Proyecto 2.
- Orbitas mas cercanas para reducir los viajes y hacer visibles los vecinos.
- Velocidad normal limitada a 420 u/s, turbo a 1.092 u/s; el frenado conserva
  autoridad al bajar la velocidad de aproximacion. Con asistencia, soltar las
  teclas detiene la nave; X tambien funciona sin asistencia.
- La franja exterior impide escapar indefinidamente. Inicio/Home recupera una
  posicion segura: es una ayuda del piloto, no parte del recorrido evaluado.

Vistas nuevas, accesibles tambien con AvPag/RePag:

```sh
cargo run --release -- --vista 22_agonia
cargo run --release -- --vista 23_jardin_biopunk
cargo run --release -- --vista 24_el_ojo
cargo run --release -- --captura capturas --vista 23_jardin_biopunk --res 1600x900
cargo run --release -- --recorrido recorrido-continuo.mp4 --res 960x540
```

La grabacion necesita `ffmpeg` en PATH o la variable `OCASO_FFMPEG` apuntando
a su ejecutable. El juego manual y las capturas PNG no lo necesitan.
Para regenerar la nave: `blender --background --python tools/export_ship.py`.
Se exportan geometría, UV, normales y los parámetros de los materiales;
la iluminación se calcula con los shaders propios de Rust.

## Qué evaluar y dónde verlo

Las vistas fijas se generan con `--captura` (nombres entre paréntesis).

| Característica del menú | Cómo está hecho | Dónde verlo |
| --- | --- | --- |
| **5 planetas** que orbitan, rotan y se ven de cerca (50) | Pira (corteza negra en placas con lava que late, acoplada por marea; la estrella la está desintegrando y le arranca una cola de polvo, como a K2-22b), Jardín (mundo vivo que respira, con una red de luz bajo la piel), Vigía (gigante gaseoso morado con un ojo-tormenta de gas rosa que gira, sigue a la nave y dilata la pupila con el latido; anillos delgados de polvo y una luna, Párpado, de cristal y lagos que reflejan el ojo), Argos (un planeta de carne con un ojo enorme que gira dentro de ella para seguir a la nave por todo el sistema, con sacadas y parpadeos; la carne en pliegues está llena de ojos chicos que también la miran) y Cementerio (anillos llenos de naves del éxodo y una baliza que sigue transmitiendo). Órbitas y giros son funciones del tiempo; las lunas giran alrededor de su planeta. | (02) a (08); volar a cualquiera |
| **9 tipos de modelos** propios en la superficie (20) | Ojo de carne (Argos), hongo y coral (Jardín), espinas de obsidiana y géiser (Pira), nave náufraga y baliza de celosía (Cementerio), racimo de cristal (Párpado) y rocas, armados con código en `scene/props.rs`, cada uno en tres niveles de detalle. | (10) a (13), (26), (28), (32) |
| **Distribución procedural** reproducible con ruido (10) | Una grilla por cara del cubo-esfera; cada celda propone un punto y lo acepta con una probabilidad de ruido fBm (grupos y claros) por las reglas del terreno (pendiente, surcos, lava, valles). Misma semilla, mismas ubicaciones. | F6 o T → semilla |
| **Posición y orientación** sobre la superficie curva que gira (5) | Cada objeto vive en el espacio de su planeta, parado sobre la normal radial y hundido en el suelo; gira con el planeta y sube y baja con la respiración de Jardín. | Cualquier superficie |
| **Shaders propios** distintos (24) | Estrella (fuego carmesí que hierve, con un ojo fijo en su superficie: un remolino alrededor de una pupila negra que late; borde en llamas que siguen en la corona, nubes de gas carmesí y magenta), Pira (placas de Voronoi y lava de cuerpo negro), Jardín (tejido con luz interna y poros), Vigía (paleta propia, vórtice de Rodrigues, brazos en espiral, iris de gas turbulento y relámpagos; del mapa del Hubble solo se toma el detalle de las nubes), Párpado (lagos espejo que reflejan a Vigía con su propio shader, cristal), Argos (iris en polares con fibras, collareta y pupila; esclera con venas; carne mojada; párpados que se cierran), Cementerio, anillos (naves y polvo), ojos de carne que miran a la nave, cristales, baliza, Pira hirviendo (corona volumétrica), nebulosa del estertor y Gargantúa. | Todas las vistas |
| **Shader animado** (8) | La estrella late: su pupila se dilata, cada latido sale de ella como un anillo de luz y su remolino gira; la lava y la luz de Jardín laten con retraso; Jardín respira; los géiseres hacen erupción; el vórtice de Vigía gira, su tormenta cambia de fuerza y de color y su pupila se dilata; el ojo de Argos parpadea, cierra la pupila con la nave cerca y sus venas laten; los cristales laten; el lago de Párpado ondula bajo la nave; los anillos orbitan con luces que parpadean; la nebulosa se expande; el gas del disco fluye. | (02), (04), (06), (10), (18), (25) |
| **Mapeo de normales** (8) | Las placas del casco de los naufragios (generadas al arrancar), la roca y el tejido orgánico en triplanar, con mipmaps; tangentes por vértice y base TBN por pixel. | (13), (05) |
| **Deformación de la geometría** con ruido (10) | Las funciones de altura de cada planeta desplazan los vértices del cubo-esfera (cráteres, crestas, mares de lava, zanjas); Jardín además respira en el vertex shader, y el de Argos aplana la carne donde está el ojo y levanta los párpados, siguiendo a la nave. | Todas; `--wire` |
| **Generación de geometría** propia antes de rasterizar (10) | Quadtree de chunks del cubo-esfera generados en paralelo con faldones, icosfera por subdivisión, modelos con `MeshBuilder`, cintas para líneas, recorte de Sutherland-Hodgman. | F3, F4 |
| **Modo de control local** (8) | Cerca de un planeta (L) el vuelo se mide contra su superficie que gira: la mira se nivela con la vertical del planeta y se transporta con la curvatura, la nave sostiene sola su altura sobre el terreno y el crucero (O) rodea el planeta. | L, O |
| **Aceleración, frenado y giro progresivos** (8) | Física a paso fijo de 120 Hz con interpolación: igual a 30 o a 300 fps. El asistente pide velocidades con aceleración limitada; el giro persigue la mira con aceleración angular limitada. | Volar |
| **Gravedad** que curva la trayectoria (10) | Gravedad del cuerpo dominante (G la apaga). Sin asistente (Z) se ve la trayectoria predicha (V), integrada con la misma física, con aviso de choque. | (15) |
| **Colisión** con el planeta (4) | Contra la misma función de altura que se dibuja; la cámara tampoco entra al terreno. | Bajar a la superficie |
| **Descarte** antes de rasterizar (7) | Esferas contra el frustum por objeto, chunks detrás del horizonte o fuera del frustum, objetos demasiado chicos, caras traseras. Contadores en el HUD (F2) y frustum congelado (F7). | F2, F7, (21) |
| **Niveles de detalle** (8) | Terreno por distancia (quadtree) y objetos por tamaño en pantalla (3 niveles, se encogen al desaparecer). F4 colorea los niveles; F5 apaga el LOD y el HUD muestra cuántos triángulos más cuesta. | F4, F5 |
| **Presentación** (10) | Dirección de arte deliberada (el final de OCASO, a pedido con J), HUD con nombres de los cuerpos, ayuda (F1) y controles explicados. | — |

Además, fuera del menú:

- **Gargantúa** ([src/shaders/gargantua.rs](src/shaders/gargantua.rs)): el gas volumétrico de mi Proyecto 2 (RK4 de las geodésicas de Schwarzschild, Doppler, corrimiento gravitacional) atravesado por cada rayo, más una tabla de mil rayos que resuelve la lente del cielo sin integrar. El gas se calcula cada dos pixeles y se interpola donde no hay bordes; en la sombra, la orilla del disco y el anillo de fotones se traza completo.
- **El ciclo** ([src/scene/cycle.rs](src/scene/cycle.rs)): una función pura del tiempo y del instante en que se presionó J, por eso se puede rebobinar.
- **Nebulosa del estertor** ([src/shaders/nebula.rs](src/shaders/nebula.rs)): una cáscara que se expande; cada rayo la corta en a lo más dos puntos, ahí se evalúan filamentos de ruido, y el borde brilla más porque se ve de canto.
- **Atmósferas** con dispersión de Rayleigh y Mie, perspectiva aérea por vértice y una tabla del cielo cuando la cámara está adentro.
- **Partículas** aditivas con profundidad: los arcos de plasma de la estrella, las fuentes de lava de los géiseres, brasas, gotas de líquido, polvo y bruma alrededor de la nave según el planeta, chispas de los naufragios, las ondas de la baliza, esporas que brillan de noche y el fuego de los motores.
- **Cola de Pira** ([src/shaders/tail.rs](src/shaders/tail.rs)): un volumen marchado a lo largo de una parábola que sale del lado de noche, lejos de la estrella; el polvo brilla más a contraluz (Henyey-Greenstein).
- **Sonido sintetizado**: el latido sincronizado con el que se ve, motor, viento y un tono por planeta.
- **Bloom** y tonemap HDR que conserva el tono.

## En la demostración: cambiar parámetros en vivo

El panel T cambia, mientras el juego corre, el radio de un planeta (se
regeneran su terreno, su atmósfera y sus objetos), su periodo orbital o de
giro (sin saltos: se corrige la fase), la intensidad del efecto de su shader,
la semilla de los objetos, el latido de la estrella, la exposición, el bloom
y el factor de LOD.

## Cómo funciona

```text
dibujos (malla + matriz + material)
 -> culling por objeto: esfera envolvente contra el frustum
 -> vértices: deformación, objeto -> mundo relativo a la cámara -> clip,
    atmósfera por vértice                                           (paralelo)
 -> triángulos: descarte, recorte, setup y reparto en tiles        (paralelo)
 -> mapa de sombras ortográfico 1024x1024, por franjas            (paralelo)
 -> raster por tiles de 64x64 al visibility buffer                (paralelo)
 -> barrera: se completa toda la profundidad antes de leer vecinos
 -> shading diferido: PBR, PCF, oclusión ambiental de pantalla      (paralelo)
 -> gas de Gargantúa cada 2 pixeles                                 (paralelo)
 -> Gargantúa, nebulosa, anillos, atmósferas y partículas           (paralelo)
 -> reflejos de pantalla en superficies mojadas cercanas           (paralelo)
 -> bloom y tonemap HDR -> RGBA8 -> textura de raylib               (paralelo)
```

- Profundidad invertida en `f32` con el plano lejano en el infinito; el
  mundo se guarda en `f64` y se dibuja relativo a la cámara.
- Un pool de hilos propio con reparto dinámico (el i5-12500H tiene núcleos
  de dos tamaños).
- El costo depende mucho de la vista (`--bench 10`). En un i5-12500H a
  1280x720, casi todas las vistas tardan entre 25 y 75 ms por frame; el
  bosque de Jardín de cerca y Gargantúa, entre 120 y 150 ms; la vista de
  cine del ojo (24), unos 300 ms.
- Todos los hilos redondean a cero los floats desnormalizados (FTZ/DAZ): un
  `exp` muy negativo deja valores tan chicos que cada operación con ellos
  cuesta cientos de ciclos.
- FXAA en CPU suaviza las diagonales despues del tonemap; F3/F4 lo omiten
  para mantener legibles las aristas y los colores de depuracion.

| Carpeta | Contenido |
| --- | --- |
| [src/render](src/render) | Pipeline: vértices, recorte, raster en punto fijo, shading diferido, partículas, bloom, tonemap y PNG |
| [src/shaders](src/shaders) | Shaders de la estrella, los planetas, los objetos, la nave, el cielo, las atmósferas, los anillos y Gargantúa; mapas de normales |
| [src/terrain](src/terrain) | Cubo-esfera con quadtree, LOD, faldones y las funciones de altura de cada planeta |
| [src/scene](src/scene) | Cuerpos celestes, ciclo, objetos de la superficie, física de la nave, trayectoria, partículas, cámara y lector OBJ |
| [src/math](src/math) | Vectores, matrices, cuaterniones, ruido, cuerpo negro, relatividad y la tabla de geodésicas |
| [src/audio.rs](src/audio.rs) | Sintetizador del sonido |
| [src/tweaks.rs](src/tweaks.rs) | Panel de ajustes en vivo |
| [src/parallel.rs](src/parallel.rs) | Pool de hilos persistentes |


### Materiales y luz de esta revisión

La nave usa los colores, texturas empacadas, rugosidad y metalicidad del
Blender original. La BRDF Cook–Torrance combina GGX, Smith y Fresnel de
Schlick. Se añaden sombras PCF y oclusión ambiental, fotografías de roca con
normales triplanares, luces locales bajo los hongos y reflejos de pantalla
en el suelo húmedo. Las luces locales y los reflejos son aproximaciones;
los reflejos solo pueden recuperar lo que está visible en el framebuffer.

Gargantúa conserva el medio **Variante** del Proyecto 2: pico de 6500 K,
disco caliente de 3 a 13 rs y envoltura gris hasta 19 rs. Los rayos que
atraviesan gas integran su espesor, absorción y emisión con el RK4 original;
el campo cilíndrico tiene 384×192×9 muestras, dos capas de flujo y rotación
kepleriana. Se conserva el Doppler y la curva de exposición del Proyecto 2.
La tabla de geodésicas acelera el cielo lejano. La vista `24_el_ojo` usa
la elevación de 2°, distancia de 21.5 rs y FOV de 34° de la vista original.
El cielo y los planetas pertenecen al sistema del Proyecto 3.

El perfil predeterminado prioriza detalle visual, también por debajo de
30 FPS. `ms_total` mide el frame completo de la app, incluyendo terreno,
sombras, actualización del gas y dibujo. El coste de carga inicial se
separa con calentamiento. Gargantúa volumétrico es la vista más costosa;
la resolución se puede bajar con `--res 960x540`.

Para reexportar la nave con Blender 5.2:

```powershell
& 'C:\Program Files\Blender Foundation\Blender 5.2\blender.exe' --background --python tools/export_ship.py
```

El exportador también escribe `ship-materials.txt` y sus seis mapas RGB8.
Las imágenes se leen, filtran y muestrean en Rust; no se usan texturas 3D
ni shaders de GPU. El archivo `.rgb` contiene ancho y alto `u32` LE,
seguidos por tres bytes RGB por pixel. Solo los mapas de color se decodifican
desde sRGB; normal, rugosidad y AO permanecen lineales.

## Integrantes y contribuciones

Trabajo individual: Lázaro Díaz (24713).

## Dependencias, recursos y asistencia

- La única dependencia es `raylib`, usada solo para la ventana, la entrada,
  mostrar el framebuffer como textura, el texto 2D del HUD y reproducir el
  sonido que genera el sintetizador. No se usa ninguna de sus funciones 3D,
  de modelos, cámara, materiales, shaders ni física.
- Reutilizo código propio: el pool de hilos, el color de cuerpo negro, la
  curva de tonemap, el bloom y la relatividad de Gargantúa vienen de mi
  Proyecto 2; el lector OBJ parte del lab 4.
- La nave OCASO-1 se creo para este proyecto en Blender con asistencia de IA
  a partir de las referencias aportadas. Su fuente esta en `assets/source/`.
  El exportador conserva UV, normales, materiales PBR y tres niveles de detalle.
  Dos láminas de efectos transparentes (campo anular y HUD holográfico) se
  excluyen del modelo opaco: evitamos que tapen el reactor como discos sólidos.
  El Blender fuente se conserva. El render, el terreno, los objetos, la
  animación del gas, el cielo y el sonido se calculan en CPU.
- Las fotografías de Júpiter y la Luna provienen de NASA; el basalto usa
  Rock Face 03 de Poly Haven (CC0). Créditos y enlaces en
  [assets/textures/FUENTES.txt](assets/textures/FUENTES.txt).
- Se usó asistencia de IA durante el desarrollo.
