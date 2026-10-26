# Cambios de la revision visual

El resto de este documento describe la arquitectura original. Esta revision
agrega los siguientes puntos para explicar en la demostracion:

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
| Pira | Lava de cuerpo negro (la temperatura da el color), grietas brillantes, acoplamiento de marea, y un chorro de brasas que cae a la estrella. |
| Jardín | Arterias que laten con la estrella, poros, capilares; la superficie respira (desplazamiento en el vertex shader). |
| Vigía | Gigante gaseoso con bandas deformadas por ruido y una tormenta en forma de ojo cuya pupila sigue a la nave. |
| Réquiem | Mares congelados, nieve en lo plano, monolitos con un ojo tallado que laten. |
| Cementerio | Cráteres y zanjas de naves estrelladas, chatarra que destella, anillos que le hacen sombra. |

## 4. Shaders ([shaders/](src/shaders))

Cada material es una función que recibe los atributos del pixel y devuelve
su color HDR. Algunos ejemplos de técnica:
- **Estrella** ([sun.rs](src/shaders/sun.rs)): celdas de convección con ruido
  deformado por otro ruido (*domain warping*), oscurecimiento del limbo y el
  latido (dos golpes por periodo).
- **Lava** ([pira.rs](src/shaders/pira.rs)): el color sale de la ley de Planck
  (temperatura → color de cuerpo negro); el ruido mueve la temperatura en el
  tiempo y forma placas de costra.
- **Jardín** ([jardin.rs](src/shaders/jardin.rs)): las arterias son las
  líneas donde un ruido cruza un valor; cuando un pixel cubre más que la
  línea, se dibuja su brillo promedio en vez de parpadear.
- **Vigía** ([vigia.rs](src/shaders/vigia.rs)): coordenadas giradas con la
  fórmula de Rodrigues para el remolino del ojo.
- **Anillos** ([rings.rs](src/shaders/rings.rs)): analíticos (intersección
  rayo-plano), con la sombra del planeta sobre ellos y la de ellos sobre el
  planeta.

**Animados**: todos los que usan `u.time` o el latido: estrella, lava, venas,
ojo, gas del disco, glifos de los monolitos.

## 5. Mapeo de normales ([shaders/textures.rs](src/shaders/textures.rs))

- Los mapas se generan al arrancar a partir de un **mapa de alturas** dibujado
  con funciones de distancia (el ojo tallado, las placas del casco). La
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

- **Modelos**: seis tipos armados con código con `MeshBuilder`
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
Quince minutos: agonía, arritmia (el latido acelera de 1.9 a 0.7 s), colapso
en caída libre (`R ∝ (1 - u²)^(2/3)`), destello, ignición del disco, Gargantúa
y renacer. Todo es función pura del tiempo. Como el latido acelera, su fase
no puede ser `t / periodo` (saltaría al cambiar el periodo): es la integral
cerrada de la frecuencia.

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
  Superficies delanteras conservan su visibilidad; la sombra del agujero
  aporta profundidad para tapar anillos y atmósferas posteriores.

### Luz y materiales (`shaders/pbr.rs`, `render/shadow.rs`)

Cook–Torrance con distribución GGX, geometría Smith y Fresnel Schlick.
Rugosidad y metalicidad provienen del Blender en la nave; fotografías y
mapas triplanares aportan detalle a los planetas. Un mapa de sombras
ortográfico 1024² rasterizado en CPU resuelve sombras cercanas con PCF
de nueve muestras. El raster completo termina antes del shading para
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
| La intensidad de su shader | Escala su efecto propio (la lava, la bioluminiscencia, el ojo, el hielo y los glifos, la chatarra). |
| La semilla | Otras posiciones para todos los objetos, siempre las mismas para esa semilla. |
| El factor de LOD | Más o menos chunks (se ve en el contador de triángulos). |
