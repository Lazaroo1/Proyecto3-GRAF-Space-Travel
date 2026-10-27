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
  nieve, polvo o bruma en un cubo que acompaña a la cámara. Las luces de los
  géiseres, las agujas y la baliza se suman a las de los hongos (las 12 más
  cercanas). N viaja de un lugar al siguiente.

# Cambios de la segunda revisión

- **El final a pedido** (`scene/cycle.rs`): ya no hay un ciclo fijo de 15
  minutos. La estrella agoniza hasta que se presiona J; desde ahí, 20 s de
  estertor, 6 s de colapso, 4 s de silencio y el ojo se abre. Todo sigue
  siendo función pura del tiempo y de los instantes de J (`Timeline`), así
  que rebobinar funciona igual. Otra J hace renacer a la estrella con un
  fundido a blanco.
- **Estrella** (`shaders/sun.rs`): granulación en tres escalas (gotas de
  plasma separadas por canales fríos, nunca negros), limbo oscuro y rojo, la
  onda de cada latido que cruza la superficie y llamaradas en el estertor. El
  color es de cuerpo negro entre 1700 y 2700 K: a 3400 K el sRGB lo vería
  color durazno.
- **Arcos de plasma** (`scene/effects.rs`): cuerdas de cinco hebras torcidas,
  amarillas en los pies y rojas arriba, con nudos que bajan; en el estertor
  crecen y uno de cada tres se desprende.
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
| Réquiem | Hielo y escarcha bajo una luz fría, grietas que respiran luz, dunas, picos de roca y monolitos con un glifo que apunta a donde nacerá Gargantúa. Un anillo de ruinas. |
| Epitafio | La luna de Réquiem: hielo viejo lleno de cráteres. |
| Cementerio | Cráteres y zanjas de naves estrelladas, anillos llenos de naves del éxodo y una baliza que sigue transmitiendo. |

## 4. Shaders ([shaders/](src/shaders))

Cada material es una función que recibe los atributos del pixel y devuelve
su color HDR. Algunos ejemplos de técnica:
- **Estrella** ([sun.rs](src/shaders/sun.rs)): granulación de plasma con ruido
  deformado por otro ruido (*domain warping*), oscurecimiento del limbo, la
  onda del latido que cruza la superficie y las llamaradas.
- **Lava** ([pira.rs](src/shaders/pira.rs)): placas de Voronoi; el color de la
  lava sale de la ley de Planck (temperatura → color de cuerpo negro). Las
  grietas más delgadas que un pixel se ensanchan y bajan su brillo en la
  misma proporción.
- **Jardín** ([jardin.rs](src/shaders/jardin.rs)): las arterias son las
  líneas donde un ruido cruza un valor; cuando un pixel cubre más que la
  línea, se dibuja su brillo promedio en vez de parpadear.
- **Vigía** ([vigia.rs](src/shaders/vigia.rs)): coordenadas giradas con la
  fórmula de Rodrigues para el vórtice; el iris en coordenadas polares
  alrededor del eje del ojo, con ruido que se retuerce y corre hacia adentro.
- **Párpado** ([parpado.rs](src/shaders/parpado.rs)): el agua refleja a
  Vigía intersectando el rayo reflejado con su esfera y llamando al shader de
  Vigía en ese punto.
- **Anillos** ([rings.rs](src/shaders/rings.rs)): analíticos (intersección
  rayo-plano), con la sombra del planeta sobre ellos y la de ellos sobre el
  planeta. Las naves son celdas de Voronoi en coordenadas polares (un número
  entero por vuelta, para que no haya costura) con una silueta de nave por
  distancia con signo; giran en bloque (con la velocidad de Kepler de cada
  radio, en un minuto se estirarían en rayas).

**Animados**: todos los que usan `u.time` o el latido: estrella, lava, venas,
vórtice, tormenta y pupila de Vigía, glifos, cristales, lagos, anillos,
nebulosa, Pira hirviendo y gas del disco.

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

- **Modelos**: once tipos armados con código con `MeshBuilder`
  ([scene/builder.rs](src/scene/builder.rs)): cajas, conos truncados,
  elipsoides y tubos con transporte paralelo. Cada uno en tres niveles de
  detalle.
- **Distribución reproducible**: cada cara del cubo se divide en una grilla;
  cada celda propone un candidato en un punto pseudoaleatorio (un hash de la
  semilla, la cara y la celda). Se acepta con una probabilidad que sale de
  **ruido fBm** (grupos y claros) por las reglas del terreno (no hay hongos en
  la lava ni monolitos en el hielo). La misma semilla da las mismas
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
  La punta de un monolito sigue iluminada cuando su base ya está de noche.

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
  nave es newtoniana.
- **Predicción de trayectoria** ([scene/trajectory.rs](src/scene/trajectory.rs)):
  se integra la caída libre 90 s hacia adelante con la misma física y se
  dibuja con cintas; si choca, se marca el punto.
- **Modo local** (L): la velocidad que se pide es relativa a la superficie
  que gira (`ω × r`); la mira se nivela con la vertical del planeta y se
  transporta con la curvatura (el giro mínimo que lleva la vertical anterior
  a la nueva), así avanzar derecho rodea el planeta por un círculo máximo; la
  altura sobre el terreno se sostiene sola mirando el suelo que viene en
  medio segundo y en un segundo. El crucero (O) empuja hacia adelante solo.
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
| La intensidad de su shader | Escala su efecto propio (la lava, la bioluminiscencia, el ojo y los cristales, el hielo y los glifos, la baliza y las luces de las naves). |
| La semilla | Otras posiciones para todos los objetos, siempre las mismas para esa semilla. |
| El factor de LOD | Más o menos chunks (se ve en el contador de triángulos). |
