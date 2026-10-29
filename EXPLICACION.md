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
| Cementerio | Cráteres y zanjas de naves estrelladas, anillos llenos de naves del éxodo y una baliza que sigue transmitiendo. |

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
- **Anillos** ([rings.rs](src/shaders/rings.rs)): analíticos (intersección
  rayo-plano), con la sombra del planeta sobre ellos y la de ellos sobre el
  planeta. La sombra del planeta tiene penumbra: su ancho es la distancia
  detrás del planeta por el radio angular de la estrella. Las naves son celdas de Voronoi en coordenadas polares (un número
  entero por vuelta, para que no haya costura) con una silueta de nave por
  distancia con signo; giran en bloque (con la velocidad de Kepler de cada
  radio, en un minuto se estirarían en rayas).

**Animados**: todos los que usan `u.time` o el latido: estrella, lava, venas,
vórtice, tormenta y pupila de Vigía, ojo de Argos, cristales, lagos, anillos,
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

- **Modelos**: nueve tipos armados con código con `MeshBuilder`
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
| La intensidad de su shader | Escala su efecto propio (la lava, la bioluminiscencia, el ojo y los cristales, el iris de Argos, la baliza y las luces de las naves). |
| La semilla | Otras posiciones para todos los objetos, siempre las mismas para esa semilla. |
| El factor de LOD | Más o menos chunks (se ve en el contador de triángulos). |
