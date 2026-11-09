# OCASO

Gráficas por Computadora · Proyecto 3 · Lázaro Díaz, 24713

Un sistema solar explorable en nave, dibujado con un renderer por software
propio en CPU. **Ocaso**, una supergigante roja, agoniza y late como un
corazón, y se puede volar entre sus planetas todo el tiempo que se quiera.
En la órbita más lejana gira su compañera muerta, **Faro**: una estrella de
neutrones que es púlsar y magnetar, a escala real y con la física de verdad
(abajo). Con **J** empieza el final: el latido se acelera, la estrella suelta
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
| ![Argos: el ojo que te sigue por todo el sistema](media/07_argos.png) | ![Faro: la magnetosfera torcida y los dos haces](media/26_haces.png) |
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
| ![El nido de Argos](media/28_nido.png) | **El nido** (Argos): carne granate y mojada, en pliegues, llena de ojos que miran a la nave y parpadean cada uno a su ritmo. Al llegar, el ojo grande viene debajo de la nave (los del nido se hunden para dejarlo pasar): se vuela encima de su iris, con la corona de ojos de sus párpados mirando desde el horizonte. Flotan gotas rojas. |
| ![El espejo en Párpado](media/32_espejo.png) | **El espejo** (Párpado, la luna de Vigía): un lago quieto entre cristales, con Vigía del otro lado del agua. El ojo te mira y también se ve en el lago, porque el reflejo se sombrea con el mismo shader de Vigía. La nave levanta ondas al pasar sobre el agua y los cristales se encienden por dentro. |
| ![El ojo de Vigía de cerca](media/29_ojo_cerca.png) | **El ojo** (Vigía): sigue a la nave y, cuando se acerca, cierra la pupila, enciende el iris y los relámpagos se juntan debajo de ella. |
| ![El claro de Jardín](media/11_hongos.png) | **El claro** (Jardín): colonias de hongos y corales que alumbran el suelo, con esporas flotando. |

## Faro, el púlsar-magnetar (F)

Faro está hecha con los números de **1E 1547.0−5408**, un magnetar que también
emite pulsos de radio (gira cada 2.07 s, campo de 3.2×10¹⁴ G), y todo lo
demás sale de ecuaciones publicadas. La estrella mide 12 km, sus casquetes
polares 130 m, sus haces nacen a 120 km y los arcos de su magnetosfera suben a
400-700 km, todo en su proporción real; se dibuja al doble (1 unidad = 500 m)
para que se vea. El tiempo también es el real. Con **N** se llega a **El
faro**, por encima de su ecuador, donde barren los haces.
El detalle, las ecuaciones y las fuentes están en
[EXPLICACION.md](EXPLICACION.md#faro-la-estrella-de-neutrones-que-es-púlsar-y-magnetar).

| | |
| --- | --- |
| ![La estrella de neutrones de cerca](media/13_estrella_de_neutrones.png) | **La estrella, a través de su propia gravedad**: la luz sigue las geodésicas de Schwarzschild (una tabla de 768 rayos integrados con RK4), así que se ve el 76% de la superficie, los dos polos a la vez, y el cielo de fondo se estira en un anillo de Einstein. La corteza de un magnetar se rompe por el estrés de su campo: placas violeta con grietas que brillan, más en los polos (Greenstein y Hartke) y en rosa y naranja en los casquetes calientes donde baja la corriente (Beloborodov 2009). |
| ![Faro de lejos, con su magnetosfera y sus haces](media/08_faro.png) | **La magnetosfera torcida**: las líneas de campo salen de la solución autosimilar de Thompson, Lyutikov y Kulkarni (2002), resuelta al arrancar por disparo y comparada con su tabla. Brillan donde hay plasma, azules junto a la estrella y violeta afuera, dibujadas como hilos de luz con núcleo y halo que se suman a la imagen. **Los haces**: conos huecos de chispas que dan vueltas (Rankin; Ruderman y Sutherland) que nacen a la altura que da la última línea abierta y se ven porque iluminan el viento de la supergigante. Cuando uno te apunta, destella y suena un golpe: dos pulsos por vuelta. |
| ![La onda de choque de la gran llamarada](media/61_faro_llamarada.png) | **La gran llamarada (F)**: la corteza tuerce la magnetosfera hasta unos 3 rad mientras tiembla cada vez más y las líneas se inflan y se encienden; hay un estallido precursor y un pico de 0.2 s que deslumbra el sistema (Parfrey, Beloborodov y Hui 2013). Las líneas se abren estirándose hacia afuera, se quedan abiertas llenas de plasma y se reconectan en tres segundos con la cima encendida, y **una onda de choque** sale por el ecuador magnético, se frena y se disuelve en violeta, doblando la imagen al pasar. |
| ![La bola de fuego atrapada](media/62_faro_bola_de_fuego.png) | Después queda una **bola de fuego** atrapada en las líneas cerradas que gira con la estrella y se evapora en seis minutos con L = L₀(1 − t/380 s)³; el giro se frena (ΔP/P = 10⁻⁴), la radio se apaga un rato y la cola suena con las oscilaciones de la corteza medidas en SGR 1806−20. Entre llamaradas hay **estallidos** de milisegundos (ley de potencias E^−1.7) que llegan en tormentas. |

Cerca de Faro el HUD muestra sus ecuaciones con sus valores (P, Ṗ, B, edad,
Ė, cilindro de luz, compacidad, torsión, casquete) y en qué va la llamarada.
Todo es función del tiempo: se rebobina con R.

## La versión chapina (V)

Con **V** el sistema entero se vuelve otro juego: Guatemala. Una cortina se
abre desde el centro de la pantalla con el borde de una faja tejida (orillas
de picos y, sobre el rojo del huipil, quetzales, volcanes, mazorcas,
barriletes y grecas mayas; al volver, una brasa) y aparece **GUATEMALA**. Las órbitas, los tamaños y la
física son los mismos, así que la nave sigue donde estaba, pero no queda nada
de OCASO a la vista: cada planeta tiene su propio relieve, su shader, sus
objetos, sus anillos, su atmósfera y su lugar; el cielo es otro (una Vía
Láctea dorada con la grieta oscura que los mayas veían como el camino a
Xibalbá), el sol es **Kinich Ajaw**, que no late ni se apaga (J no hace
nada), la nave es una **botella** o un **vehículo chapín** y en vez del latido suena **música**. Es
de día: la luz del sol es más blanca y fuerte que la de la gigante roja y
las sombras tienen más relleno.

| | |
| --- | --- |
| ![Kinich Ajaw con el glifo k'in](media/53_kinich.png) | ![Antigua, el Arco y el Volcán de Fuego](media/42_antigua.png) |
| ![Tikal: el Templo del Gran Jaguar sobre la selva](media/44_tikal.png) | ![Panajachel y los volcanes de Atitlán](media/46_panajachel.png) |
| ![Semuc Champey: pozas turquesa](media/47_semuc.png) | ![San Andrés Xecul y la Luna de Xelajú](media/49_xecul.png) |
| ![La capital y su anillo de camionetas](media/51_capital.png) | ![La Torre del Reformador](media/52_reformador.png) |
| ![La columna de ceniza del Fuego sobre Antigua](media/56_columna_de_ceniza.png) | ![El mercado frente a la iglesia de San Andrés Xecul](media/57_mercado_de_xecul.png) |
| ![Torres de oficinas alrededor del Reformador](media/58_torres_de_la_capital.png) | |

| OCASO | Versión chapina | Lugar (N) |
| --- | --- | --- |
| La estrella | **Kinich Ajaw**, el sol maya: fuego dorado con el glifo k'in (la flor de cuatro pétalos) fijo en su superficie, corona dorada con rayos | — |
| Pira | **Volcán de Fuego**: el Fuego con la cumbre de ceniza oscura llena de surcos, barrancas de ceniza gris que bajan entre el bosque, coladas de lava con costra negra que se resquebraja y brilla por las grietas, y una **columna de ceniza** que sale con cada explosión y se la lleva el viento; a su lado el Acatenango, dormido, con bosque hasta arriba; en los valles, aldeas y cafetales | **Antigua**: el Arco de Santa Catalina con su reloj (las agujas avanzan), casas coloniales, alfombras de aserrín en la calle, árboles en los patios, camionetas en las avenidas y un mercado en la calle que cruza |
| Jardín | **Petén**: selva, ríos y lagos | **Tikal**: el Templo del Gran Jaguar saliendo sobre la selva, ceibas y luciérnagas |
| Vigía | **Atitlán**: un mundo lago con los volcanes Atitlán, Tolimán y San Pedro, con bosque casi hasta la punta y barrancos; el agua refleja y en las tardes el Xocomil la pica | **Panajachel**: el muelle y las lanchas que se mecen con las olas |
| Párpado | **Semuc Champey**: pozas turquesa en terrazas de caliza entre la selva | **Las pozas** |
| Argos | **Los Cuchumatanes**: sierra con helada al amanecer y pinos; en su anillo, los barriletes gigantes de Sumpango | **San Andrés Xecul**: la iglesia amarilla con sus columnas de colores, ángeles, enredaderas y cúpulas de gajos (todo pintado por el shader), en un pueblo con plaza empedrada, mercado de puestos con toldos de colores, casas y camionetas |
| — | **La Luna de Xelajú**, luna de los Cuchumatanes, con un tejido de huipil que brilla de noche | **La marimba**: una marimba doble gigante sobre el polvo, con su faldón típico; ahí se toca el minijuego (Enter) |
| — | **La capital**: barrancos, cuadras y luces; en su anillo, camionetas y tuc-tucs dando vueltas | **El Reformador**: la torre de celosía como la de la foto (patas abiertas con arcos sobre la 7a Avenida, alas con círculos, torrecitas en el primer nivel, un círculo en cada tramo del fuste, campana y faro), lo más alto del centro; camionetas extraurbanas pasando abajo, edificios y torres de oficinas de vidrio |
| Faro | — (en la versión chapina no existe) | — |

**Los vehículos** (modelados en Blender para este proyecto, a partir de
fotos de referencia): el **Tuk-Tuk Elena** de San Juan, **La Mamaneta**
(una camioneta extraurbana con su parrilla de luces) y el **Colectivo Q.5**,
con **7**, **8** y **9**. Sus colores salen del `.mtl` y, del nombre de cada
material, de qué es la pieza: las LEDs brillan con su luz, el cromo y los
vidrios reflejan el cielo y la pintura lleva laca.

| | | |
| --- | --- | --- |
| ![El Tuk-Tuk Elena](media/66_tuk_tuk.png) | ![La Mamaneta](media/67_mamaneta.png) | ![El Colectivo Q.5](media/68_colectivo.png) |

**Las naves botella** (modeladas en Blender para este proyecto): Gallo,
Cabro Reserva, Jägermeister, Blue Label, Cubata Uva y Four Loko. **B** pasa a
la siguiente y **1** a **6** eligen una; cada una echa algo distinto (espuma
la cerveza, fuego verde el Jäger, azul el Blue Label, morado la Cubata,
chispas de colores el Four Loko).

**La música**: las canciones van en una carpeta `musica/` junto al proyecto
(mp3, ogg, wav; no se suben al repositorio) y suenan en orden aleatorio, con un **crossfade** de 3.5 s
entre una y otra; un `primera.txt` con el nombre de una canción (o un
pedazo) hace que siempre empiece con esa. Al final de cada vuelta está una
**marimba procedural** escrita en código (síntesis modal de las teclas
afinadas 1:4:10, el charleo de los tecomates y el redoble, en 6/8 con
sesquialtera), a la que se llega con U / I. Abajo a la derecha hay un mini reproductor: **K** pausa,
**U** / **I** canción anterior / siguiente, **-** / **+** volumen, y con el
mouse suelto (Tab) se les puede dar clic a los botones y a la barra.

## Minijuegos

Cada planeta va a tener su minijuego. Cerca de su lugar, abajo aparece
**«Enter · …»** y Enter lo abre; Esc vuelve al vuelo. Mientras se juega, el
mundo se queda quieto: la última imagen del render se congela de fondo,
difuminada, y el juego se dibuja encima en CPU como todo lo demás, a los
fps de la pantalla (de 3 a 13 ms por frame según el juego; el 3D del
sistema cuesta 50-140).

| | |
| --- | --- |
| ![La marimba de la Luna](media/65_marimba.png) | **La marimba de la Luna** (Luna de Xelajú, versión chapina; con N se llega a **La marimba**). Fichas que bajan por cuatro carriles hasta las teclas de una marimba: **D F J K**, como Piano Tiles. Vos llevás una parte del conjunto y la marimba toca lo demás: **Bajo** (fácil), **Tiple** (la melodía) o **Tiple con redobles** (difícil: las notas largas se redoblan machacando la tecla, como en la marimba de verdad; si dejás de redoblar, la nota se apaga). Lo que no tocás no suena, y se oye. La canción es el son de la marimba procedural en **tres vueltas**, cada una más rápida (119, 139 y 158 por minuto). Perfecto, bien, ok o falla según el tiempo; la racha multiplica y, si el **ánimo** del público se acaba, el público se va. Al final: «¡De a huevo!», «¡Chilero!», «Va pues», «Púchica…» o «Ni modo, otra vez», y el récord de cada parte. |
| ![La carrera del Periférico](media/69_carrera.png) | **La carrera del Periférico** (la capital, versión chapina; se ofrece en el aire sobre la capital: con N se llega a **El Periférico**, arriba de la ciudad con el anillo de camionetas encima). Tres vueltas a una carretera mojada que flota a 400 m sobre la capital de noche (barrancos, calzadas, La Aurora y el estadio), con los volcanes en el horizonte y el Fuego en erupción, la meta bajo la Torre del Reformador (que también es el semáforo), túneles de arcos de foquitos de feria, barriletes gigantes de Sumpango, cohetes, túmulos y barandas de neón que se reflejan en el asfalto. Corrés con la nave que traés y las rivales son las otras tres clases: **la chela** (la botella que traés, o la Gallo), **el tuk-tuk**, **la camioneta** y **el colectivo**, cada una con su manejo (el tuk-tuk arranca como bala y dobla de lado; la camioneta corre más en recta pero dobla mal y echa **humo negro** que te nubla la vista). Ir pegado atrás de otra nave carga el **turbo** (rebufo, Shift); **H** es el **pito**: el tráfico de adelante se hace a un lado. Las naves son tus modelos de Blender **horneados como sprites** al empezar, con LEDs que corren, respiran y se encienden al frenar, ladeo, rebote y su turbo. Hay récord por nave. `Jugar_Carrera.cmd` (o `--carrera`) arranca ahí. |
| ![Shucos del Reformador](media/70_shucos.png) | **Shucos del Reformador** (la capital, versión chapina; se ofrece en el suelo, junto a la Torre del Reformador: con N se llega a **El Reformador**). Una carretilla al pie de la torre, de 7 de la noche a 3 de la mañana. Llegan clientes con su pedido en un globito: **el de la U**, **el agente de la PMT** (con prisa), **la señora** que lo quiere «sin cebolla y bastante guacamol», **el del estadio** (doble salsa de tomate) y, después de la una, **el bolo de las 2 a. m.**; y cada noche, a eso de las 8 y cuarto, llega **don Meyer** (el de Talega Surfers) saludando a pedir el suyo. Arriba del mostrador **te va diciendo qué apachar en cada paso** para el cliente elegido. Armás el shuco: **1** el pan, **2-5** la carne (salchicha, longaniza, chorizo, salami: la primera vez a la plancha, la segunda al pan; cruda o quemada te la devuelven), **G R C M Y T** guacamol, repollo, cebolla, mostaza, mayonesa y salsa de tomate, **X** a la basura, **← →** a quién y **Enter** se lo das. Mal armado te lo devuelven; si tardás se van enojados; si sale rápido, dejan propina. Hay horas pico (la salida de la U y la del estadio) y con los ahorros comprás mejoras: plancha de hierro, toldo y ayudante. Todo con volumen y la luz de los focos de la carretilla; la torre, los edificios y la avenida desenfocados detrás, con sus luces como manchas, y por la avenida pasan carros, el Tuk-Tuk Elena y La Mamaneta pitando. `Jugar_Shucos.cmd` (o `--shucos`) arranca ahí. |
| ![Talega Surfers](media/71_surfers.png) | **Talega Surfers** (Antigua, en el volcán de Fuego, versión chapina; con N se llega a **Antigua**). Te recibe **don Meyer de frente, con su saludo militar**. **Meyer** (el personaje de Blender, con su gorra, sus lentes, su guerrera beige y la cara con textura) se baja de la nave y corre saludando por la calle del Arco, en tres carriles de empedrado, entre fachadas coloniales de colores (con sus aleros, rejas voladas y zaguanes), bajo el Arco de Santa Catalina y con el Volcán de Agua al fondo, con el sol de la tarde que tira la sombra de las casas sobre la calle, iglesias y jacarandas detrás de los techos, gente en las banquetas (el de los globos, señoras con su canasta, turistas) y palomas que salen volando. Las camionetas son tu **La Mamaneta** (en tres pinturas) y los tuk-tuks tu **Tuk-Tuk Elena**, con sus LEDs encendidas, derechas en su carril y **en fila, como un convoy**: a las **paradas** se sube corriendo por su **rampa de tablones** y se sigue por los techos de toda la fila (sin rampa, se esquivan); las que **vienen de frente** no perdonan (salvo que vayás por los techos); **abajo** se agacha (en cuclillas); las camionetas y tuk-tuks **de cerca van en 3D** (su malla en perspectiva: a la par se les ve el costado y desde la rampa se ven largas), los de lejos como sprites; los **tuk-tuks** se esquivan, las **carretas de helados** se saltan y los **lazos de papel picado** se pasan rodando. Si pisás una **alfombra de aserrín**, te persiguen los **cucuruchos** (con su marcha de procesión) y si pisás otra te agarran. Atrás **el Fuego** va explotando: la ceniza oscurece todo y caen **bombas de lava** que marcan su sombra antes de caer. Los **lenes** son las monedas; cada vez se corre más rápido. **H** es el claxon de don Meyer. De fondo suena *The British Grenadiers* con pífano y tambor, desde la bienvenida y, más bajita, mientras se corre. Hay récord. `Jugar_Surfers.cmd` (o `--surfers`) arranca ahí. |
| ![La marimba en la Luna de Xelajú](media/64_la_marimba.png) | **Para jugar sin buscarla**: `Jugar_Marimba.cmd` (o `cargo run --release -- --marimba`) arranca con la marimba abierta. En el menú, ← / → ajustan el tiempo si las notas te suenan tarde (audífonos Bluetooth). Los récords quedan en `minijuegos.txt`. Cómo funciona (la pista en perspectiva hecha fila por fila, el reloj que sale del audio, el redoble): [EXPLICACION.md](EXPLICACION.md#los-minijuegos). |

## Video

**[El video de muestra](media/muestra.mp4)** (4 min 12 s, con sonido): la
ventana del juego tal cual, con su HUD, manejada por un guion
(`--muestra`). Pasa con **N** por cada lugar de OCASO (el claro de Jardín,
los géiseres de Pira, el nido de Argos, el espejo de Párpado y Faro, con su
gran llamarada) con el crucero rodeando el planeta; el final de la estrella
(el estertor, el colapso, la onda gravitacional, Gargantúa comiéndose a Pira
y El Ojo); la **V** a la versión chapina, y otra vez **N** por cada lugar
(Antigua, Tikal, Panajachel, San Andrés Xecul, El Reformador, Semuc, la
Luna de Xelajú y El Periférico), abriendo con **Enter** y jugando solos los
cuatro minijuegos: Talega Surfers, los shucos, la marimba y la carrera. El
tiempo avanza fijo (1/30 s por frame), así que el video va en tiempo real
aunque cada frame se tarde lo suyo; el sonido es el que genera el juego (el
sintetizador y el de los minijuegos; las canciones del reproductor no).

Y dos recorridos continuos, uno por versión, con todo lo que pide el
requisito 5: **vuelo espacial, acercamiento, una vuelta completa al planeta
y alejamiento**, sin cortes ni cambio de escena:

- [OCASO](media/recorrido-ocaso.mp4) (32 s): desde el espacio hasta Jardín,
  la vuelta sobre el bosque de hongos (de noche y de día, con la estrella
  encima) y la salida hasta ver el planeta entero con Vigía al fondo.
- [Versión chapina](media/recorrido-chapina.mp4) (20 s): Petén con la
  Gallo, sobre las ceibas, el amanecer a media vuelta y la salida.

Los graba el programa sin ventana (`--recorrido`, y `--chapina` para la
versión chapina): un piloto automático vuela con la misma física y los
mismos controles que el vuelo manual (acelera hacia el planeta, frena, pasa
al modo local, rodea el planeta con turbo hasta completar los 360° y se
aleja). Se exportan a 24 fps con la simulación acelerada x3, así que no son
una medición de FPS en tiempo real.

Para grabar uno a mano en la demostración (F9 oculta el HUD):

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
cargo run --release -- --chapina              # empieza en la versión chapina
cargo run --release -- --recorrido video.mp4  # graba el recorrido (ffmpeg); con --chapina, el chapín
cargo run --release -- --muestra muestra.mp4  # graba el video de muestra: la ventana manejada por un guion (ffmpeg)
cargo run --release -- --captura capturas     # las 57 vistas fijas como PNG, sin abrir ventana
cargo run --release -- --marimba              # arranca en la Luna de Xelajú con la marimba abierta
cargo run --release -- --nave 8               # la nave chapina con que se empieza (7 tuk-tuk, 8 camioneta, 9 colectivo)
cargo run --release -- --captura-marimba fotos # fotos del minijuego de la marimba, sin ventana
cargo run --release -- --carrera              # arranca en la capital con la carrera del Periférico abierta
cargo run --release -- --captura-carrera fotos # fotos de la carrera (con piloto automático), sin ventana
cargo run --release -- --shucos               # arranca en El Reformador con la carretilla de shucos abierta
cargo run --release -- --captura-shucos fotos # fotos de los shucos (con piloto automático), sin ventana
cargo run --release -- --surfers              # arranca en Antigua con Talega Surfers abierta
cargo run --release -- --captura-surfers fotos # fotos de Talega Surfers (con piloto automático), sin ventana
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
| **Recuperar posición** junto a Pira, conservando el ciclo | **H** (o Inicio / Home) |
| **Viajar al siguiente lugar** (claro, géiseres, nido, espejo y el faro; en la versión chapina se llega de día: la marimba de la Luna, Antigua (Talega Surfers), El Reformador (los shucos), El Periférico (la carrera) y los demás) | **N** |
| **Minijuego** del planeta, cuando abajo dice «Enter · …» | **Enter** |
| En la marimba: tocar / elegir parte / ajuste de tiempo / tocar o repetir / salir | D F J K / ↑ ↓ / ← → / Enter / Esc |
| En la carrera: manejar / turbo / pito / arrancar / salir | Flechas o W A S D / Shift / H / Enter / Esc |
| En los shucos: pan / carnes / guacamol, repollo, cebolla, mostaza, mayonesa, tomate / tirar / cliente / entregar | 1 / 2 3 4 5 / G R C M Y T / X / ← → / Enter |
| En Talega Surfers: carril / saltar / agacharse (en el aire, bajar de golpe) / claxon de don Meyer / correr / salir | ← → o A D / ↑ o W / ↓ o S / H / Enter / Esc |
| Acercar / alejar la cámara | Rueda |
| Cámara de persecución / cabina | C |
| Asistente de vuelo / gravedad | Z / G |
| **Modo local** (vuelo relativo a la superficie) | L |
| **Crucero**: rodea el planeta solo | O |
| **Ruta predicha**: la línea de a dónde te lleva la gravedad (auto: solo sin asistencia / siempre / nunca) | Y |
| **Versión chapina** / volver a OCASO | **V** |
| Nave chapina siguiente / elegir una (1 a 6 botellas, 7 tuk-tuk, 8 camioneta, 9 colectivo) | B / 1 a 9 |
| Música: pausa, anterior / siguiente, volumen (en la chapina) | K, U / I, - / + |
| Rebobinar el tiempo | R sostenido |
| Saltar 30 s atrás / adelante en el tiempo | [ / ] |
| **Desatar el final** de la estrella / hacerla renacer | **J** |
| **Gran llamarada** de Faro | **F** |
| Recorrer las vistas fijas (las mismas de `--captura`) | AvPág / RePág, o **.** / **,** en las laptops |
| **Ajustes en vivo** (radio, órbita, semilla, shaders...) | T, flechas y Shift |
| Wireframe / colores de LOD / LOD encendido | F3 / F4 / F5 |
| Otra semilla para los objetos de la superficie | F6 |
| Congelar el frustum (ver el culling desde afuera) | F7 |
| Resolución dinámica (encendida: mientras la imagen se mueve rápido se renderiza al 75 % y se amplía) | F8 |
| Silenciar | M |
| Foto en `capturas/` | P |
| Soltar o capturar el mouse | Tab |
| Ayuda / estadísticas / ocultar todo el HUD | F1 / F2 / F9 |
| Salir (en un minijuego, Esc sale del minijuego) | Esc |


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
  teclas detiene la nave; sin asistencia sigue de largo, pero los
  propulsores no la pasan de la velocidad maxima. X tambien funciona sin
  asistencia.
- La franja exterior impide escapar indefinidamente. H (o Inicio/Home) recupera una
  posicion segura: es una ayuda del piloto, no parte del recorrido evaluado.

Vistas nuevas, accesibles tambien con AvPag/RePag (o el punto y la coma):

```sh
cargo run --release -- --vista 22_agonia
cargo run --release -- --vista 23_jardin_biopunk
cargo run --release -- --vista 24_el_ojo
cargo run --release -- --captura capturas --vista 23_jardin_biopunk --res 1600x900
cargo run --release -- --recorrido media/recorrido-ocaso.mp4
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
| **5 planetas** que orbitan, rotan y se ven de cerca (50) | Pira (corteza negra en placas con lava que late, acoplada por marea; la estrella la está desintegrando y le arranca una cola de polvo, como a K2-22b), Jardín (mundo vivo que respira, con una red de luz bajo la piel), Vigía (gigante gaseoso morado con un ojo-tormenta de gas rosa que gira, sigue a la nave y dilata la pupila con el latido; anillos delgados de polvo y una luna, Párpado, de cristal y lagos que reflejan el ojo), Argos (un planeta de carne con un ojo enorme que gira dentro de ella para seguir a la nave por todo el sistema, con sacadas, parpadeos y una corona de ojos en sus párpados; volando bajo va debajo de la nave; la carne en pliegues está llena de ojos chicos que también la miran) y Párpado, la luna de Vigía, con su propio terreno y su lugar. Además **Faro**, la estrella de neutrones (arriba), en la órbita más lejana. Órbitas y giros son funciones del tiempo; las lunas giran alrededor de su planeta. | (02) a (08), (13); volar a cualquiera |
| **21 tipos de modelos** propios en la superficie (20) | En OCASO: ojo de carne (Argos), hongo y coral (Jardín), espinas de obsidiana y géiser (Pira), racimo de cristal (Párpado) y rocas, armados con código en `scene/props.rs`. En la versión chapina, 14 más (el Arco, casas, el Templo del Gran Jaguar, ceibas, árboles, lanchas, la iglesia de San Andrés Xecul, pinos, puestos, camionetas, la Torre del Reformador, edificios, torres de oficinas y la marimba de la Luna), en `chapin/`. Cada uno en tres niveles de detalle. | (10) a (12), (28), (32), (42) a (58), (64) |
| **Distribución procedural** reproducible con ruido (10) | Una grilla por cara del cubo-esfera; cada celda propone un punto y lo acepta con una probabilidad de ruido fBm (grupos y claros) por las reglas del terreno (pendiente, surcos, lava, valles). Misma semilla, mismas ubicaciones. | F6 o T → semilla |
| **Posición y orientación** sobre la superficie curva que gira (5) | Cada objeto vive en el espacio de su planeta, parado sobre la normal radial y hundido en el suelo; gira con el planeta y sube y baja con la respiración de Jardín. | Cualquier superficie |
| **Shaders propios** distintos (24) | Estrella (fuego carmesí que hierve, con un ojo fijo en su superficie: un remolino alrededor de una pupila negra que late; borde en llamas que siguen en la corona, nubes de gas carmesí y magenta), Pira (placas de Voronoi y lava de cuerpo negro), Jardín (tejido con luz interna y poros), Vigía (bandas por latitud que corren a su ritmo con bordes turbulentos y ondas de Kelvin-Helmholtz, vórtice de Rodrigues, brazos en espiral, iris de gas turbulento, tormentas eléctricas con rayos adentro, auroras en los polos que laten con la estrella y relámpagos por todo el lado de noche; del mapa del Hubble solo se toma el detalle de las nubes), Párpado (lagos espejo que reflejan a Vigía con su propio shader, cristal), Argos (iris en polares con fibras, collareta y pupila; esclera con venas; carne mojada; párpados que se cierran), Faro (la corteza agrietada vista a través de su lente, con la ley de Greenstein-Hartke y los casquetes de Beloborodov; sus haces como volumen con el perfil de Rankin, el carrusel de chispas y hebras que corren; la bruma de su viento con integral exacta; el cielo curvado alrededor), anillos (los de Vigía como los de Saturno: zonas, divisiones, franjas de colores, lunitas pastoras con sus olas, un anillo de grumos trenzado y rayos radiales), ojos de carne que miran a la nave, cristales, Pira hirviendo (corona volumétrica), nebulosa del estertor y Gargantúa. | Todas las vistas |
| **Shader animado** (8) | La estrella late: su pupila se dilata, cada latido sale de ella como un anillo de luz y su remolino gira; la lava y la luz de Jardín laten con retraso; Jardín respira; los géiseres hacen erupción; el vórtice de Vigía gira, su tormenta cambia de fuerza y de color y su pupila se dilata; el ojo de Argos parpadea, cierra la pupila con la nave cerca y sus venas laten; los cristales laten; el lago de Párpado ondula bajo la nave; los anillos orbitan; la nebulosa se expande; el gas del disco fluye; los haces de Faro barren el sistema con chispas que dan vueltas y cada pulso distinto, su plasma sube por las líneas y parpadea, y sus casquetes se calientan con los estallidos y la llamarada. | (02), (04), (06), (10), (18), (25), (26), (60) a (62) |
| **Mapeo de normales** (8) | Las placas del casco de la nave (generadas al arrancar), la roca y el tejido orgánico en triplanar, con mipmaps; tangentes por vértice y base TBN por pixel. | (09), (05), (12) |
| **Deformación de la geometría** con ruido (10) | Las funciones de altura de cada planeta desplazan los vértices del cubo-esfera (cráteres, crestas, mares de lava, zanjas); Jardín además respira en el vertex shader, y el de Argos aplana la carne donde está el ojo y levanta los párpados, siguiendo a la nave. | Todas; `--wire` |
| **Generación de geometría** propia antes de rasterizar (10) | Quadtree de chunks del cubo-esfera generados en paralelo con faldones, icosfera por subdivisión, modelos con `MeshBuilder`, cintas para líneas (la trayectoria y las líneas de campo de Faro, rehechas cada frame), recorte de Sutherland-Hodgman. | F3, F4 |
| **Modo de control local** (8) | Cerca de un planeta (L) el vuelo se mide contra su superficie que gira: la mira se nivela con la vertical del planeta y se transporta con la curvatura, la nave sostiene sola su altura sobre el terreno y el crucero (O) rodea el planeta. | L, O |
| **Aceleración, frenado y giro progresivos** (8) | Física a paso fijo de 120 Hz con interpolación: igual a 30 o a 300 fps. El asistente pide velocidades con aceleración limitada; el giro persigue la mira con aceleración angular limitada. | Volar |
| **Gravedad** que curva la trayectoria (10) | Gravedad del cuerpo dominante (G la apaga). Sin asistente (Z) se ve la trayectoria predicha (Y), integrada con la misma física, con aviso de choque. | (15) |
| **Colisión** con el planeta (4) | Contra la misma función de altura que se dibuja; la cámara tampoco entra al terreno. | Bajar a la superficie |
| **Descarte** antes de rasterizar (7) | Esferas contra el frustum por objeto, chunks detrás del horizonte o fuera del frustum, objetos demasiado chicos, caras traseras. Contadores en el HUD (F2) y frustum congelado (F7). | F2, F7, (21) |
| **Niveles de detalle** (8) | Terreno por distancia (quadtree) y objetos por tamaño en pantalla (3 niveles, se encogen al desaparecer). F4 colorea los niveles; F5 apaga el LOD y el HUD muestra cuántos triángulos más cuesta. | F4, F5 |
| **Presentación** (10) | Dirección de arte deliberada (el final de OCASO, a pedido con J), HUD con nombres de los cuerpos, ayuda (F1) y controles explicados. | — |

Además, fuera del menú:

- **Minijuegos** ([src/minijuegos/](src/minijuegos)): la marimba de la Luna, con la pista en perspectiva dibujada fila por fila en CPU (cada fila es un instante de la canción), la partitura del son repartida entre quien juega y la marimba, y el reloj sacado del audio para que lo que se ve vaya con lo que se oye; y la carrera del Periférico, un pseudo-3D con lomas y curvas, con los modelos de Blender horneados como sprites por un rasterizador propio (con mips y LEDs animadas), la ciudad como una textura horneada vista por un rayo contra la esfera, el asfalto mojado que refleja lo que emite y un posproceso con bloom de dos escalas, todo en CPU; y los shucos del Reformador, una escena 2D de figuras de distancia con signo que se sombrean con volumen (la normal sale de la distancia al borde: Lambert, Blinn-Phong y contraluz), con el fondo desenfocado como foto y sus luces como manchas, y don Meyer horneado de frente; y Talega Surfers, con un rayo por pixel contra el suelo, las fachadas, el Arco y la luz de la tarde con sus sombras, las camionetas de cerca dibujadas con un rasterizador propio en perspectiva ([src/minijuegos/malla.rs](src/minijuegos/malla.rs): recorte contra el plano cercano, perspectiva correcta y profundidad contra la calle), y encima, con su profundidad, las rampas, las monedas, las bombas y los personajes.
- **Gargantúa** ([src/shaders/gargantua.rs](src/shaders/gargantua.rs)): el gas volumétrico de mi Proyecto 2 (RK4 de las geodésicas de Schwarzschild, Doppler, corrimiento gravitacional) atravesado por cada rayo, más una tabla de mil rayos que resuelve la lente del cielo sin integrar. El gas se calcula cada dos pixeles y se interpola donde no hay bordes; en la sombra, la orilla del disco y el anillo de fotones se traza completo.
- **Faro** ([src/scene/faro.rs](src/scene/faro.rs), [src/scene/magnetosphere.rs](src/scene/magnetosphere.rs), [src/shaders/faro.rs](src/shaders/faro.rs), [src/math/neutron.rs](src/math/neutron.rs)): la estrella de neutrones de arriba; su lente, su magnetosfera, sus estallidos y su llamarada son funciones puras del tiempo, así que también se rebobinan.
- **El ciclo** ([src/scene/cycle.rs](src/scene/cycle.rs)): una función pura del tiempo y del instante en que se presionó J, por eso se puede rebobinar.
- **Nebulosa del estertor** ([src/shaders/nebula.rs](src/shaders/nebula.rs)): una cáscara que se expande; cada rayo la corta en a lo más dos puntos, ahí se evalúan filamentos de ruido, y el borde brilla más porque se ve de canto.
- **Atmósferas** con dispersión de Rayleigh y Mie, perspectiva aérea por vértice y una tabla del cielo cuando la cámara está adentro.
- **Hilos de luz** ([src/render/glow.rs](src/render/glow.rs)): segmentos aditivos con núcleo y halo, probados contra la profundidad; son las líneas de campo de Faro.
- **Partículas** aditivas con profundidad: los arcos de plasma de la estrella, las fuentes de lava de los géiseres, brasas, gotas de líquido y destellos alrededor de la nave según el planeta, la bola de fuego de Faro, el destello de sus haces, esporas que brillan de noche y el fuego de los motores.
- **Cola de Pira** ([src/shaders/tail.rs](src/shaders/tail.rs)): un volumen marchado a lo largo de una parábola que sale del lado de noche, lejos de la estrella; el polvo brilla más a contraluz (Henyey-Greenstein).
- **Sonido sintetizado**: el latido sincronizado con el que se ve, motor, viento, un tono por planeta y Faro (un golpe por cada haz que pasa por la nave, el chisporroteo de los estallidos, el trueno de la llamarada y las oscilaciones de su corteza).
- **Bloom** y tonemap HDR que conserva el tono.
- **Resolución dinámica** (F8, encendida en la ventana): mientras la imagen se mueve rápido (lo que gira la cámara más lo que avanza sobre la superficie más cercana, en rad/s, con histéresis) el frame se renderiza al 75 % y se amplía con Catmull-Rom bicúbico: de 1.5 a 1.65 veces los FPS volando cerca de los planetas. Quieta, las fotos (P), las capturas y el video, siempre a resolución completa; F2 dice a qué tamaño va.
- **Optimizaciones sin bajar la calidad**: el ruido de Worley se salta las celdas que ya no pueden ganar (mismo resultado, 24 % más rápido) y Faro calcula una vez por frame lo de sus haces y tabula la desviación de su luz lejos de la estrella. En Gargantúa, los pasos del RK4 en el vacío (lejos del gas y del anillo de fotones) son el doble de largos sin que crezca el error contra una referencia más fina, y los rayos que ya van cayendo se cortan: sus vistas, de 13 a 16 % más rápidas. Se midió con contadores de ciclos por material y por etapa (ver [EXPLICACION.md](EXPLICACION.md)).

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
| [src/shaders](src/shaders) | Shaders de la estrella, los planetas, Faro, los objetos, la nave, el cielo, las atmósferas, los anillos y Gargantúa; mapas de normales |
| [src/terrain](src/terrain) | Cubo-esfera con quadtree, LOD, faldones y las funciones de altura de cada planeta |
| [src/scene](src/scene) | Cuerpos celestes, ciclo, la física de Faro y su magnetosfera, objetos de la superficie, física de la nave, trayectoria, partículas, cámara y lector OBJ |
| [src/math](src/math) | Vectores, matrices, cuaterniones, ruido, cuerpo negro, relatividad y las tablas de geodésicas (Gargantúa y Faro) |
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
- Las seis naves botella las modelé en Blender para la versión chapina, a
  partir de fotos de referencia de las botellas y latas reales; sus `.blend`
  y versiones detalladas están fuera del repositorio y aquí van las de menos
  de 30 000 triángulos (`assets/botellas/`). Las marcas son de sus dueños.
- Los tres vehículos (el Tuk-Tuk Elena, La Mamaneta y el Colectivo Q.5) los
  modelé en Blender a partir de fotos de referencia; aquí van sus versiones
  media (14 000 a 28 000 triángulos) y baja, en `assets/naves/`.
- El personaje de don Meyer (`assets/personajes/`) se modeló en Blender con
  asistencia de IA a partir de una foto de referencia; la cara es una
  textura proyectada de esa foto (las gafas se quitaron de la imagen con un
  generador de imágenes y se modelaron aparte). Sus poses (correr, caminar,
  saltar, agacharse) se arman en código girando sus piezas.
- El claxon de don Meyer (H en Talega Surfers) es un audio grabado,
  `assets/sonidos/claxon_meyer.wav`, que se lee con un lector de WAV propio.
- La canción de fondo de Talega Surfers es una grabación de *The British
  Grenadiers* (marcha tradicional del siglo XVIII) con pífano y tambor,
  `assets/sonidos/british_grenadiers.wav`, pasada a mono.
- Las canciones de `musica/` no se incluyen en el repositorio (pesan y tienen
  derechos de autor); la marimba procedural sí es código propio.
- Las fotografías de Júpiter y la Luna provienen de NASA; el basalto usa
  Rock Face 03 de Poly Haven (CC0). Créditos y enlaces en
  [assets/textures/FUENTES.txt](assets/textures/FUENTES.txt).
- Se usó asistencia de IA durante el desarrollo.
