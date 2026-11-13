# Cahier des charges

# Prebench — Simulateur de cartes de développement

9 novembre 2026 · @Mathis

## 0. Mode d'emploi du document

Ce document est la référence unique du projet : il décrit ce que l'app doit faire, comment elle est construite et ce qui est volontairement exclu. Il sert deux lecteurs : moi, pour ne rien oublier et retrouver la réponse à chaque question, et une IA générative, à qui il peut être transmis tel quel pour coder l'app.

- **Prebench** est le nom de travail de l'application. « Paillasse » désigne le canevas de câblage à l'intérieur de l'app.
- Le document ne contient que des décisions **validées**. Ce qui reste à trancher est regroupé dans la section 15 (Questions ouvertes).
- Les mots **DOIT**, **NE DOIT PAS** et **PEUT** ont un sens contractuel : DOIT = obligatoire, NE DOIT PAS = interdit, PEUT = optionnel.
- Les priorités sont notées **V1** (premier produit utilisable), **V2** (seconde vague) et **Plus tard** (prévu dans l'architecture, pas planifié).
- Dans le dépôt, ce document vit dans `docs/cahier-des-charges.md` ; le fichier `CLAUDE.md` à la racine y renvoie.
- Pour une IA : lire d'abord les sections 1, 2 et 16, puis la section correspondant à la tâche demandée.

## 1. Vision et objectifs

Prebench est une application de bureau locale qui permet de programmer et tester un projet Nucleo (STM32) ou ESP32 comme si la carte et ses composants étaient câblés sur la paillasse, avant même de les avoir reçus.

**Problème résolu.** Le développement d'un projet embarqué est bloqué tant que les composants ne sont pas livrés. Prebench permet de commencer immédiatement : écrire le firmware, le câbler virtuellement, interagir avec les composants et lire la sortie série.

**Objectifs**

1. Simuler une carte Nucleo ou ESP32 et ses entrées/sorties, sans simuler le cœur du processeur, son horloge ni ses registres.
2. Offrir un nombre illimité de composants d'interface grâce à un système extensible, plus des composants génériques (« jokers »).
3. Laisser l'utilisateur coder dans son éditeur préféré : l'app est la paillasse, pas l'IDE.
4. Rester légère, sobre en CPU et en RAM, et incapable de surcharger l'ordinateur.
5. Fonctionner entièrement hors ligne, sur macOS (Apple Silicon) et Windows, et être téléchargeable librement sur GitHub sous forme de versions publiées.
6. Faciliter le passage au vrai matériel : même code source, liste de câblage et nomenclature exportées.

**Règle directrice.** Le simulateur valide la logique, la paillasse réelle valide la physique. Tout ce qui est logique (états, protocoles applicatifs, réseau, interface, réveils) est simulé ; ce qui relève du timing précis, de l'analogique ou de la dynamique physique ne l'est pas, ou seulement de façon simplifiée.

## 2. Décisions validées

Ces choix structurent tout le projet ; les remettre en cause revient à changer d'architecture.

| # | Décision | Raison | Alternative écartée |
| --- | --- | --- | --- |
| D1 | Simulation au niveau de l'API (« shim ») : le firmware est compilé pour l'ordinateur hôte et lié à une bibliothèque qui remplace le HAL | Simple à développer, exécution native très rapide, couvre tous les périphériques voulus | Émulation du binaire réel (Renode, QEMU) : trop lourde, couverture partielle des périphériques |
| D2 | Cibles : cartes Nucleo (STM32) et ESP32 | Besoin réel du projet | Autres familles : possibles plus tard par simple fichier de carte |
| D3 | Une API interne unique (libvmcu) + des shims fins par framework (STM32 HAL, ESP-IDF, Arduino) | Ajouter un framework = traduire, pas réécrire | Un simulateur par puce |
| D4 | Composants extensibles décrits par fichiers (manifeste + modèle + vue) + composants jokers. Un composant vit dans le projet (`<projet>/components/<id>/`, prioritaire) ou parmi les composants fournis ; l'app installée compile elle-même modèles et vues, sans Node ni outil externe | Composants « sans limite » sans toucher au cœur | Catalogue codé en dur |
| D5 | Modèles de composants en TypeScript, exécutés dans un moteur JavaScript embarqué au cœur Rust (QuickJS via `rquickjs`), un runtime isolé par instance ; `model.ts` compilé par le cœur (`oxc`) | Langage maîtrisé ; latence d'un aller-retour synchrone mesurée en phase 1 : 99e centile 1,2 ms et pics à 9 ms via WebView + Web Worker, contre moins de 0,1 ms dans le cœur ; simulation et future CLI indépendantes de la fenêtre | Web Workers dans la WebView (mesurés trop lents), Rust ou WASM pour les composants |
| D6 | Stack Tauri 2 : cœur Rust + interface web (Svelte, SVG, Canvas) | App légère (moteur web du système), multiplateforme | Electron (lourd), SwiftUI (Mac seulement) |
| D7 | Firmware exécuté dans un processus séparé, relié à l'app par socket local + mémoire partagée | Un plantage du firmware ne fait pas tomber l'app ; garde-fous système possibles | Firmware chargé dans l'app |
| D8 | Réseau électrique logique (nets à états), pas de simulation analogique | Suffisant pour pull-up, boutons, bus partagés, courts-circuits | SPICE |
| D9 | Pas d'éditeur de code intégré : l'app surveille le dossier du projet | L'utilisateur garde son éditeur ; rien à importer | Refaire un IDE |
| D10 | 100 % hors ligne | Aucune dépendance à un service | Moteur en ligne type Wokwi |
| D11 | macOS + Windows dès le départ, Linux possible | Le coût d'un portage tardif serait bien plus élevé | Mac uniquement |
| D12 | Trois « créneaux d'avenir » figés dès la V1 : identifiant de carte dans chaque message, canal binaire de flux, tout le temps qui passe par libvmcu | Rendent possibles multi-cartes, audio/caméra et temps virtuel sans refonte | Les ajouter plus tard |
| D13 | Distribution open source « écrasée » : dépôt privé prebench-dev pour le travail quotidien, dépôt public prebench avec un commit par version et les binaires en GitHub Releases (section 12.5) | Historique privé, code et composants ouverts aux contributions, CI de release gratuite dans le dépôt public | Binaires seuls en freeware : CI macOS coûteuse en dépôt privé, libvmcu à distribuer de toute façon |
| D14 | Premier framework simulé : STM32 HAL. Cartes V1 : Nucleo-F401RE et ESP32-DevKitC | La feuille de route part d'un blinky STM32 HAL ; ce sont les cartes les plus répandues de chaque famille | Arduino ou ESP-IDF en premier |
| D15 | Protocole en trames binaires à en-tête fixe, little-endian | Aucune dépendance côté C (C11), simple à déboguer | MessagePack |
| D16 | Licence MIT pour tout le projet : application, libvmcu, shims, boards/ et components/ | La plus simple et la plus répandue ; les utilisateurs compilent libvmcu avec leur propre firmware sans contrainte ; compatible avec FreeRTOS (MIT) | GPL-3.0 pour l'application (empêche une version fermée, mais freine l'adoption) ; Apache-2.0 (plus lourde, sans intérêt ici) |

## 3. Périmètre

La V1 vise une carte Nucleo et une ESP32 avec GPIO, PWM, ADC, UART, I2C et SPI ; tout ce qui dépend du timing réel, de l'analogique ou de la physique est exclu.

### 3.1 Cibles

- **Nucleo (STM32)** : framework STM32 HAL, premier framework simulé (code généré par CubeMX accepté), Arduino (STM32duino) en option.
- **ESP32** (famille ESP32, S3, C3) : framework ESP-IDF, Arduino-ESP32 en option.
- Cartes V1 : Nucleo-F401RE et ESP32-DevKitC (D14). Toute autre carte s'ajoute par un fichier de définition (section 7).

### 3.2 Inclus

| Fonction | Exemples | Priorité |
| --- | --- | --- |
| GPIO sortie | LED, LED RGB, buzzer actif, relais | V1 |
| GPIO entrée + interruptions | Bouton, interrupteur, fin de course, PIR | V1 |
| PWM | LED variable, servomoteur, moteur, buzzer passif | V1 |
| ADC | Potentiomètre, LDR, joystick, capteur analogique | V1 |
| UART | Terminal série, GPS, module AT | V1 |
| I2C | OLED SSD1306, MPU6050, BME280, RTC, PCF8574 | V1 |
| SPI | TFT ILI9341 / ST7735 (résolution réglable) | V1 |
| Shims de bibliothèques tierces | NeoPixel / FastLED / led_strip, DHT | V1 (WS2812) puis V2 |
| Matrices | Clavier 4×4, matrice LED, 7 segments multiplexés | V2 |
| FreeRTOS | Port POSIX (Mac), port Windows | V1 pour ESP-IDF |
| Jauge mémoire de la vraie cible | RAM / flash via compilation croisée | V1 |
| Wi-Fi via les sockets de l'hôte | MQTT, serveur web embarqué | V2 (prioritaire) |
| Multi-cartes | UART croisé, CAN, passerelle Nucleo + ESP32 | V2 |
| Audio I2S | Haut-parleurs et micro de l'ordinateur | V2 |
| Carte SD et systèmes de fichiers | FatFS, LittleFS, SPIFFS, NVS dans le dossier projet | V2 |
| BLE au niveau GATT | Panneau « téléphone virtuel » | Plus tard |
| Basse consommation | États de veille, sources de réveil | Plus tard |
| Temps virtuel + modèles physiques | PID, robot, four, moteur + encodeur | Plus tard |
| Caméra, USB | esp_camera via webcam, TinyUSB HID / CDC | Plus tard, à la demande |

### 3.3 Hors périmètre (assumé)

- Simulation du cœur, de l'horloge et des registres du microcontrôleur.
- Code accédant directement aux registres (`GPIOA->ODR`), macros LL, DMA, timers matériels avancés.
- Temps réel dur : bit-banging maison, signal VGA, fréquencemètre, oscilloscope à DMA, commande FOC.
- Électronique analogique (SPICE) : filtres, conditionnement, courant dans une LED. Exception : un composant « pont diviseur » simplifié.
- Bootloader, écriture en flash bas niveau, OTA réel (seule l'API `esp_ota_*` vers un fichier est envisageable).
- Wi-Fi et BLE au niveau radio, estimation de durée de batterie.

Pour ces cas, la réponse est la vraie carte, ou Renode ponctuellement à côté de l'app.

## 4. Architecture technique

L'app est un exécutable Tauri 2 : un cœur Rust qui orchestre et route, une interface web qui dessine la paillasse et exécute les modèles de composants, et un processus firmware par carte simulée.

[embed: node/f1e98d45-497b]

Le firmware ne voit que son framework habituel ; tout ce qui sort de libvmcu passe par le socket ou la mémoire partagée, puis le cœur Rust envoie l'état à l'interface et les transactions aux modèles de composants.

### 4.1 Stack

| Couche | Technologie | Rôle |
| --- | --- | --- |
| Coquille de l'app | Tauri 2 (WKWebView sur macOS, WebView2 sur Windows) | Fenêtre native, IPC, capabilities de sécurité |
| Cœur | Rust | Gestion des processus firmware, routage des messages, réseau logique, surveillance des ressources, build, projet |
| Interface | Svelte + SVG (paillasse) + Canvas (écrans) | Canevas, panneaux, inspecteur |
| Modèles de composants | TypeScript compilé par `oxc` et exécuté par QuickJS (`rquickjs`) dans le cœur Rust : un fil « modèles » par carte, un runtime par instance (16 Mo, pile 256 Ko, 5 ms par appel, 50 ms au chargement) | Comportement des composants, isolé |
| Visuels de composants | wokwi-elements (web components) si la licence le permet, sinon SVG maison | LED, boutons, écrans, potentiomètres |
| Vues de composants | `view.svelte` compilé à l'exécution dans la WebView par le compilateur Svelte embarqué ; vue de repli si la compilation échoue | Rendu des composants fournis et du projet |
| Côté firmware | libvmcu (C) + shims (STM32 HAL, ESP-IDF, Arduino) | Remplace le HAL, parle à l'app |
| Build du firmware | CMake + clang (Mac) / clang ou MinGW-w64 (Windows) ; `arm-none-eabi-gcc` et toolchain Xtensa pour la jauge mémoire | Cible « sim » et cible réelle |
| Crates Rust prévues | `interprocess`, `shared_memory`, `sysinfo`, `cpal` (audio), `nokhwa` (caméra), `notify` (surveillance de fichiers), `rquickjs` (modèles de composants), `oxc` (compilation de `model.ts`) | Abstraction multiplateforme |

### 4.2 Principes d'architecture

- Le cœur Rust est **purement événementiel** : aucune boucle de polling, 0 % de CPU sans simulation.
- Chaque firmware tourne dans **son propre processus**, lancé et surveillé par le cœur.
- Le cœur reçoit tous les événements, tient le réseau logique et n'envoie à l'interface que l'état utile à l'affichage, au rythme des images.
- Tout ce qui dépend du système d'exploitation vit dans un module `platform/` (Rust) et `vmcu_platform_*.c`(libvmcu). Le reste du code n'appelle jamais une API système directement.

### 4.3 Organisation du dépôt

```
prebench/
├── CLAUDE.md            # règles pour l'IA, renvoie vers docs/cahier-des-charges.md
├── LICENSE              # MIT (D16)
├── README.md
├── CHANGELOG.md
├── release.sh           # publication vers le dépôt public (section 12.5)
├── .gitattributes       # export-ignore des fichiers internes
├── .github/workflows/   # ci.yml (privé : Windows + Linux), release.yml (public)
├── app/                 # Tauri : src-tauri (Rust) + ui (Svelte)
│   ├── src-tauri/src/
│   │   ├── core/        # orchestrateur, routage, réseau logique
│   │   ├── platform/    # macos.rs, windows.rs
│   │   ├── build/       # détection et lancement du build
│   │   └── project/     # lecture/écriture prebench.toml
│   └── ui/              # canevas, panneaux, runtime des composants
├── libvmcu/             # bibliothèque C côté firmware
│   ├── include/vmcu.h   # API publique vmcu_*
│   ├── core/            # client du socket, horloge, garde-fous
│   ├── platform/        # vmcu_platform_posix.c, vmcu_platform_win32.c
│   └── shims/           # stm32-hal/, esp-idf/, arduino/, libs/ (NeoPixel, DHT…)
├── boards/              # fichiers de définition de cartes (.toml)
├── components/          # un dossier par composant
├── templates/           # projets de départ
├── cli/                 # prebench run / prebench test
└── docs/
    ├── cahier-des-charges.md
    └── protocol.md      # spécification du protocole versionnée
```

## 5. libvmcu et shims

Le firmware de l'utilisateur reste strictement identique : seule la cible de compilation change (`make sim` / build « sim » pour l'ordinateur, build normal pour la carte).

### 5.1 Principe

1. Le firmware est compilé pour l'hôte (clang arm64 sur Mac, clang/MinGW sur Windows).
2. Il est lié à un shim qui expose les mêmes fonctions que le vrai framework (`HAL_GPIO_WritePin`, `gpio_set_level`, `digitalWrite`…).
3. Le shim traduit chaque appel vers l'API interne `vmcu_*` de libvmcu.
4. libvmcu envoie un message à l'app ; dans l'autre sens, elle reçoit les événements (bouton pressé, valeur d'un capteur) et déclenche les callbacks.

```c
// shims/stm32-hal/stm32_hal_gpio.c
void HAL_GPIO_WritePin(GPIO_TypeDef *port, uint16_t pin, GPIO_PinState s) {
    vmcu_gpio_write(port_id(port), pin_index(pin), s);
}
GPIO_PinState HAL_GPIO_ReadPin(GPIO_TypeDef *port, uint16_t pin) {
    return vmcu_gpio_read(port_id(port), pin_index(pin));
}
void     HAL_Delay(uint32_t ms) { vmcu_delay_ms(ms); }
uint32_t HAL_GetTick(void)      { return vmcu_millis(); }
```

### 5.2 API interne libvmcu (contrat central)

| Famille | Fonctions (résumé de l'API v1) |
| --- | --- |
| Cycle de vie | `vmcu_init(board, framework)`, `vmcu_shutdown()` ; l'identifiant d'instance de carte (D12) est fourni par l'app (`PREBENCH_BOARD_ID`) |
| Temps | `vmcu_millis()`, `vmcu_micros()`, `vmcu_delay_ms()`, `vmcu_delay_us()` — **tout le temps passe par ici**(D12) |
| Sections critiques | `vmcu_irq_lock()`, `vmcu_irq_unlock()` : empêchent l'exécution des callbacks, équivalent de `__disable_irq` / `taskENTER_CRITICAL` |
| GPIO | `vmcu_gpio_config(port, pin, mode, pull, af)` (`af` = fonction alternative STM32, ou `VMCU_GPIO_AF_NONE`), `vmcu_gpio_write()`, `vmcu_gpio_read()`, `vmcu_gpio_on_edge(port, pin, edge, cb, user)` |
| PWM | `vmcu_pwm_config(unit, channel, freq_hz)` (STM32 : unité = timer), `vmcu_pwm_set_duty(unit, channel, duty)`, `vmcu_pwm_attach(unit, channel, port, pin)` |
| ADC | `vmcu_adc_config(unit, channel)`, `vmcu_adc_read(unit, channel)` → valeur brute selon la résolution de la carte (cache local, sans aller-retour) |
| UART | `vmcu_uart_config(id, baud)`, `vmcu_uart_write(id, buf, len)`, `vmcu_uart_read(id, buf, max, timeout_ms)`, `vmcu_uart_on_rx(id, cb, user)` |
| I2C | `vmcu_i2c_config(bus, freq_hz)`, `vmcu_i2c_xfer(bus, addr, tx, txlen, rx, rxlen, timeout_ms)` → ACK/NACK |
| SPI | `vmcu_spi_config(bus, mode, freq_hz, lsb_first)`, `vmcu_spi_xfer(bus, cs_port, cs_pin, tx, rx, len, timeout_ms)` |
| Flux | `vmcu_stream_open(kind, params, &stream)` (audio, framebuffer, caméra) → tampon en mémoire partagée |
| Diagnostic | `vmcu_warn_unsimulated("HAL_xxx")`, `vmcu_log()`, `vmcu_ping(target, &rtt_us)` : aller-retour synchrone firmware ↔ cœur (`VMCU_PING_CORE`) ou firmware ↔ cœur ↔ modèle d'un composant d'écho (`VMCU_PING_WORKER`), section 8.1 |

L'API v1 est figée en phase 0 dans `libvmcu/include/vmcu.h`, qui fait foi ; ce tableau en est un résumé.

### 5.3 Exigences des shims

- Toute fonction du framework non implémentée **DOIT compiler** et afficher « ⚠ HAL_xxx non simulé » à l'exécution, jamais une erreur de lien.
- STM32 HAL : `SystemClock_Config`, `HAL_RCC_*`, `MX_*_Init` et `HAL_Init` sont acceptés comme fonctions sans effet ; les types CMSIS sont conservés.
- ESP-IDF : `app_main` tourne dans une tâche FreeRTOS ; FreeRTOS est fourni par son port POSIX (Mac) ou Windows. NVS devient un fichier du projet.
- Arduino : un shim commun aux deux familles (`digitalWrite`, `analogRead`, `Wire`, `SPI`, `Serial`, `delay`, `millis`).
- Les callbacks d'interruption (`HAL_GPIO_EXTI_Callback`, ISR ESP-IDF) sont appelés depuis un thread de libvmcu.

### 5.4 Shims de bibliothèques tierces

Les protocoles qui dépendent du timing (WS2812, DHT11/22, HC-SR04, 1-Wire, IR) ne peuvent pas être simulés au niveau du signal. Ils sont simulés **au niveau de la bibliothèque** : `Adafruit_NeoPixel`, FastLED, `led_strip` / RMT, bibliothèques DHT, etc. Le dépôt prévoit `libvmcu/shims/libs/` pour ces remplacements dès la V1.

### 5.5 Garde-fous intégrés à libvmcu

- Les délais (`HAL_Delay`, `delay`, `vTaskDelay`) sont de vrais sommeils.
- Détection d'attente active : après des milliers de lectures d'une broche sans changement, micro-pause d'environ 1 ms.
- Limiteur pour les `while(1)` sans délai : micro-pause tous les N appels à libvmcu.
- La tâche idle de FreeRTOS dort.
- libvmcu quitte immédiatement si le socket vers l'app se ferme (aucun processus orphelin).
- Sous Windows, délais par timers haute résolution (`CREATE_WAITABLE_TIMER_HIGH_RESOLUTION`), sinon `Sleep(1)`dure environ 15 ms.

## 6. Protocole firmware ↔ app

Deux canaux : un socket local pour les messages courts, binaires et groupés, et de la mémoire partagée pour les gros flux. Le protocole est le contrat dont dépendent tous les composants : il est figé tôt, versionné et documenté dans `docs/protocol.md`.

### 6.1 Canal messages

- Transport : socket Unix (macOS) / named pipe (Windows), via la crate `interprocess`.
- Format **binaire** en trames à en-tête fixe, little-endian (D15). Le JSON est réservé aux fichiers de configuration, jamais au chemin critique.
- Chaque message porte au minimum : version du protocole (à la poignée de main), **identifiant de carte** (D12), type, horodatage `vmcu_micros()`, charge utile.
- Les messages sont **groupés par lots** ; une transaction de bus = un message (une écriture SPI entière, pas un octet).

| Sens | Types de messages |
| --- | --- |
| Firmware → app | `hello` (carte, framework, version), `gpio_config`, `gpio_write`, `pwm_config`, `pwm_duty`, `uart_tx`, `i2c_xfer`(requête), `spi_xfer` (requête), `stream_open`, `warn`, `log` |
| App → firmware | `gpio_input` (changement d'état d'une entrée), `adc_value`, `uart_rx`, `i2c_reply` (données + ACK/NACK), `spi_reply`, `reset`, `pause`, `resume`, `shutdown` |

Les transactions I2C et SPI sont **synchrones** : le firmware attend la réponse du composant modélisé, comme sur un vrai bus.

### 6.2 Canal flux (mémoire partagée)

- Tampons circulaires de taille fixe, via la crate `shared_memory`.
- Usages : framebuffers d'écrans (indicateur « modifié »), audio I2S, caméra.
- Ordre de grandeur à respecter : une trame TFT 320×240 en 16 bits pèse environ 150 Ko et ne doit jamais transiter octet par octet.

### 6.3 Contre-pression

- Si l'interface ne suit pas, des images d'affichage sont sautées.
- Les données logiques (états, transactions, série) ne sont jamais bloquées ni perdues ; elles sont conservées dans des tampons circulaires bornés.

## 7. Fichiers de définition de cartes

Une carte est un fichier TOML dans `boards/`, sans aucune émulation : ajouter une carte, c'est ajouter un fichier.

```toml
#:schema ./board.schema.json
# boards/nucleo-f401re.toml (extrait)
format_version = 1
name      = "Nucleo-F401RE"
mcu       = "STM32F401RE"
family    = "stm32"
framework = ["stm32-hal", "arduino"]
logic_voltage = 3.3
adc_bits  = 12
ram_kb    = 96
flash_kb  = 512
image     = "nucleo-64.svg"
reset     = "B2"

[power]
"3V3" = 3.3
"5V"  = 5.0
GND   = 0.0

[onboard.LD2]
type   = "led"
params = { color = "green" }
wires  = { A = "PA5", K = "GND" }

[onboard.B1]
type  = "button"
wires = { "1" = "PC13", "2" = "GND" }

[pins.PA5]
label   = "D13"
caps    = ["gpio", "exti", "pwm:TIM2_CH1", "spi1:SCK"]
af      = { "pwm:TIM2_CH1" = 1, "spi1:SCK" = 5 }
onboard = "led:LD2"

[pins.PC13]
label         = "B1"
caps          = ["gpio", "exti"]
onboard       = "button:B1"
external_pull = "up"

[pins.PB8]
label = "D15"
caps  = ["gpio", "exti", "i2c1:SCL"]
af    = { "i2c1:SCL" = 4 }
```

### 7.1 Contenu obligatoire

Le format est défini par `boards/board.schema.json` (format v1), qui fait foi. Les noms de broches suivent la règle du protocole : `PA5` (STM32, port = lettre − A) ou `GPIO5` (ESP32).

- Identité : nom, microcontrôleur, famille, frameworks supportés.
- Caractéristiques utiles à la simulation : tension logique, résolution ADC, RAM et flash (pour la jauge mémoire).
- Alimentations `[power]` : nom → tension (`3V3`, `5V`, `GND`), utilisables comme extrémités de câble (`board.GND`).
- Brochage `[pins.<nom>]` : étiquette de connecteur, capacités (`gpio`, `exti`, `adcN:CHx`, `pwm:TIMx_CHy`, `i2cN:SDA|SCL`, `spiN:…`, `uartN:TX|RX`) et, facultatifs :
  - `af` : numéro de fonction alternative de chaque capacité (STM32), pour savoir quelle fonction porte une broche en mode alternatif (PA5 : TIM2_CH1 en AF1, SPI1_SCK en AF5) ;
  - `external_pull` : résistance de tirage soudée sur la carte (`up` / `down`) ;
  - `constraints` : `input-only`, `strapping`, `flash` (ESP32, section 7.2) ;
  - `onboard` : rappel du composant intégré relié à la broche (`led:LD2`).
- Composants intégrés `[onboard.<id>]` : type de composant (`components/<type>/`), paramètres et câblage interne (broche du composant → broche ou alimentation de la carte). Ce ne sont que des composants ordinaires (règle 16.2.7).
- `reset` : étiquette du bouton reset intégré, représenté par le bouton Reset de la barre d'outils ; `image` : image SVG de la carte.

### 7.2 Règles propres à chaque famille

- **STM32** : fonctions alternatives fixes par broche. Le câblage est validé contre `caps`.
- **ESP32** : matrice GPIO (la plupart des périphériques sur presque toute broche), avec des contraintes déclarées dans le fichier : GPIO 34 à 39 en entrée uniquement, broches de strapping, broches du flash interdites. L'app affiche un avertissement, comme un collègue sur la paillasse.

### 7.3 Cartes de départ

La Nucleo-F401RE et l'ESP32-DevKitC (ESP32 classique) en V1 (D14). Les cartes suivantes (Nucleo-L476, ESP32-C3, autres) ne demandent qu'un fichier.

## 8. Système de composants

« Sans limite » signifie : tout composant s'ajoute sous forme de dossier sans modifier le cœur, et cinq composants jokers couvrent les puces que personne n'a encore modélisées.

### 8.1 Anatomie d'un composant

```
components/ssd1306/
├── manifest.json   ← broches, interfaces (i2c @0x3C), paramètres (128x64 / 128x32)
├── model.ts        ← comportement : reçoit les messages de bus, met à jour son état
└── view.svelte     ← rendu (SVG ou Canvas)
```

- **manifest.json** : identifiant, nom, catégorie, broches et leur rôle, interface(s) et paramètres (adresse I2C, résolution, nombre de LED…), contrôles exposés à l'utilisateur (curseurs, boutons), version de l'API composant.
- **model.ts** : implémente l'interface TypeScript v1 (`components/component-api.d.ts`) : `onPinChange`, `onI2C(addr, tx) → rx`, `onSPI(tx) → rx`, `onUART(bytes)`, `onTick(dt)`, `onControl`, `onPing`, `getState()`. Compilé par le cœur (`oxc`, types retirés fichier par fichier) et exécuté dans QuickJS, un runtime par instance, sur le fil « modèles » de la carte : sans accès au disque, au réseau ni au système. Limites : 16 Mo de mémoire, 256 Ko de pile, 50 ms pour charger et initialiser le module, 5 ms par appel (bus, `onPinChange`, `onControl`, `onTick`) ; au-delà, le composant passe en défaut avec un avertissement ⚠, les autres continuent.
- **view.svelte** : dessine l'état, compilé à l'exécution dans la WebView ; reçoit l'état une fois par image (« sim:states »), seulement si `invalidate()` a été appelé, et renvoie les actions de l'utilisateur au modèle (`component_control`). Vue de repli (nom, broches, état brut, contrôles) si la vue manque ou ne compile pas.
- Rechargement à chaud des composants pendant le développement.

**Latence mesurée en phase 1.** Les transactions I2C et SPI sont synchrones (section 6.1) : le firmware attend la réponse du modèle. Avec les modèles dans des Web Workers (firmware → cœur → WebView → Worker → firmware), la mesure dans l'app (Mac, debug) donnait une médiane de 326 µs, un 99e centile de 1,2 ms et des pics à 9 ms, dus à la WebView. Les modèles s'exécutent donc dans le cœur Rust (D5) : le chemin devient firmware → cœur → fil « modèles » → firmware, et la simulation ne dépend plus de l'état de la fenêtre (section 11.3).

Mesures dans l'app (Mac, build debug, outil `tests/latency`, 2000 allers-retours par chemin) :

| Chemin | Médiane | 99e centile | Maximum |
| --- | --- | --- | --- |
| firmware ↔ cœur (référence) | 17 µs | 40 µs | — |
| firmware ↔ cœur ↔ WebView ↔ Web Worker (avant) | 326 µs | 1193 µs | 8886 µs |
| firmware ↔ cœur ↔ modèle QuickJS (décision D5) | 33 µs | 260 µs | 1269 µs |

Mesures sous Windows et Linux (runners GitHub Actions, build debug, test `latency_tool`, 2000 allers-retours par chemin, 11 novembre 2026) :

| Chemin | Système | Médiane | 99e centile | Maximum |
| --- | --- | --- | --- | --- |
| firmware ↔ cœur | Windows | 35 µs | 131 µs | 421 µs |
| firmware ↔ cœur ↔ modèle QuickJS | Windows | 60 µs | 134 µs | 353 µs |
| firmware ↔ cœur | Linux | 36 µs | 46 µs | 107 µs |
| firmware ↔ cœur ↔ modèle QuickJS | Linux | 68 µs | 83 µs | 1906 µs |

Les runners sont des machines virtuelles partagées : ces valeurs majorent celles d'un PC physique.

Le 99e centile passe sous 0,3 ms ; le maximum reste au-dessus de 1 ms, probablement dû à l'ordonnancement du système (firmware en priorité basse, cause non mesurée).

### 8.2 Familles d'interfaces

| Interface | Exemples | Difficulté | Point d'attention |
| --- | --- | --- | --- |
| GPIO sortie | LED, buzzer actif, relais | Triviale |  |
| GPIO entrée | Bouton, interrupteur, PIR | Triviale | Pull-up/pull-down, anti-rebond optionnel |
| PWM | LED variable, servo, moteur, buzzer passif | Facile | On simule le rapport cyclique, pas le signal |
| ADC | Potentiomètre, LDR, joystick | Facile | Curseur ou générateur de forme d'onde |
| UART | Terminal, GPS, module AT (HC-05, SIM800) | Facile à moyenne | Modules AT via répondeur scriptable |
| I2C | OLED, MPU6050, BME280, RTC, PCF8574 | Moyenne | Une table de registres par puce |
| SPI | TFT ILI9341 / ST7735, carte SD, NRF24 | Moyenne à lourde | Carte SD : shim FatFS plutôt que protocole SD |
| Matrices | Clavier 4×4, matrice LED, 7 segments | Moyenne | Exige un réseau logique correct |
| Protocoles à timing | WS2812, DHT, HC-SR04, 1-Wire, IR | Piège | Shim de bibliothèque (section 5.4) |

### 8.3 Composants jokers

Configurables depuis l'interface, sans écrire de code :

1. **Périphérique I2C générique** : adresse au choix + table de registres éditable (valeurs fixes, curseurs, scripts).
2. **Périphérique SPI générique** : même principe, avec trace des trames.
3. **Périphérique UART générique** : écho, réponses scriptées (`AT` → `OK`), fichier de rejeu.
4. **Source analogique** : constante, curseur, sinus, bruit, fichier CSV.
5. **Sonde et générateur numériques** : afficher un signal ou injecter une séquence.

### 8.4 Catalogue initial (V1)

Ces dix composants valident l'architecture, car ils couvrent toutes les familles, pièges compris :

- LED (+ LED RGB)
- Bouton poussoir
- Potentiomètre
- Buzzer
- Servomoteur
- OLED SSD1306 (I2C, 128×64 / 128×32)
- MPU6050 (I2C, valeurs réglables par curseurs)
- TFT ILI9341 (SPI, framebuffer en mémoire partagée)
- Ruban WS2812 (via shim NeoPixel / led_strip)
- Joker I2C

### 8.5 Environnement

Un panneau « environnement » permet de régler les grandeurs physiques lues par les capteurs : température, humidité, luminosité, inclinaison et accélération de l'IMU, distance.

## 9. Réseau électrique logique et validations

Chaque câble appartient à un « net » dont l'état est calculé par le cœur Rust ; il n'y a aucune simulation analogique.

### 9.1 États d'un net

| État | Signification |
| --- | --- |
| Piloté haut | Une sortie impose 1 |
| Piloté bas | Une sortie impose 0 |
| Tiré haut / tiré bas | Seule une résistance de pull (interne ou composant) agit |
| Flottant | Rien ne fixe le niveau : lecture indéterminée + avertissement |
| Conflit | Deux sorties imposent des niveaux opposés : court-circuit signalé |

Résistances et ponts diviseurs sont décoratifs ou simplifiés ; un composant « pont diviseur » peut fournir une tension calculée à l'ADC.

### 9.2 Validations affichées dans l'onglet ⚠

- Broche incompatible avec la fonction câblée (SDA sur une broche sans I2C pour un STM32).
- Contraintes ESP32 : GPIO en entrée seule, broches de strapping, broches du flash.
- Adresse I2C en double, broche CS SPI partagée.
- Sortie contre sortie (conflit), entrée flottante.
- Niveaux 3,3 V / 5 V incompatibles.
- Fonction du framework non simulée.
- Débit série différent entre le firmware et le moniteur.

## 10. Flux de travail et interface

L'utilisateur code dans son éditeur ; l'app surveille le dossier, recompile la cible « sim » à chaque sauvegarde et recharge le firmware à chaud sans perdre l'état de la paillasse.

### 10.1 Projet

- Ouvrir un projet = ouvrir un **dossier**. Rien n'est importé ni copié.
- Le build « sim » se fait dans le cache de l'app (`<cache>/builds/<nom>-<empreinte du chemin>`), jamais dans le dossier du projet ni dans celui de l'installation, en lecture seule sous Windows. Il est supprimé quand la configuration CMake échoue, pour qu'un cache corrompu ne bloque pas le build suivant ; une erreur de compilation du code le garde, pour que la correction ne relance pas une recompilation complète.
- Le dossier contient le code de l'utilisateur et un fichier `prebench.toml` (carte, composants, paramètres, câblage), versionné dans Git avec le code.
- Composants propres au projet : `<projet>/components/<id>/`, prioritaires sur les composants fournis portant le même identifiant (avertissement ⚠). Quand ce dossier existe, l'app y écrit `component-api.d.ts` (en-tête « généré par Prebench, ne pas modifier » et version de l'API), réécrit s'il diffère de la version de l'app.
- Détection automatique du système de build : CMake, Makefile, PlatformIO, ESP-IDF, sketch Arduino.
- Modèles de nouveaux projets : blinky Nucleo HAL, ESP-IDF, Arduino. **Nouveau projet** = copie d'un modèle dans un dossier de l'utilisateur (`Documents/Prebench/<modèle>` proposé), sans les builds locaux ; le modèle installé n'est jamais modifié. Au démarrage, l'app ouvre la copie du blinky si elle existe ; sinon, elle n'ouvre pas le modèle installé (non modifiable) mais propose « Créer mon premier projet », qui ouvre le formulaire Nouveau projet avec le blinky présélectionné.
- Chemins transmis aux outils de build sans le préfixe Windows `\\?\` (illisible pour make) ; les chemins avec espaces (`C:\Program Files`) sont pris en charge.

```toml
# prebench.toml (esquisse)
board = "nucleo-f401re"
framework = "stm32-hal"

[[components]]
id = "oled1"
type = "ssd1306"
params = { address = 0x3C, size = "128x64" }
pos = [420, 180]

[[wires]]
from = "oled1.SDA"
to = "board.PB9"
```

### 10.2 Cycle édition → simulation

1. L'utilisateur sauvegarde dans son éditeur.
2. L'app détecte le changement et lance le build « sim » (et le build de la vraie cible pour la jauge mémoire).
3. En cas d'erreur : liste cliquable qui ouvre `fichier:ligne` dans l'éditeur (schéma `vscode://file/…:ligne`, sinon application par défaut du fichier ; choix d'un autre éditeur : Plus tard). L'ancien firmware continue de tourner jusqu'au build réussi suivant.
4. En cas de succès : rechargement du firmware ; câblage et valeurs des capteurs conservés.
5. Débogage : l'app écrit dans `.vscode/launch.json` une configuration qui lance le firmware sous le débogueur (CodeLLDB sur Mac ; extension C/C++ et gdb de MSYS2 sous Windows) et l'attend sans délai ; elle signale l'extension ou le débogueur absent. Quand le débogueur s'arrête, toute la paillasse se fige (« Arrêté sur un point d'arrêt ») ; à la reprise, le temps du firmware repart sans saut (protocole 1.1, `docs/protocol.md` section 5.5).

### 10.3 Disposition de l'écran

```
┌───────────────────────────────────────────────────────────────────┐
│ ▶ ⏸ ⟲ Reset   ⏻ Alim   Nucleo-F401RE ▾   ×1   ● CPU 4%  RAM 38 Mo │
├───────────────────────────────────────────────────┬───────────────┤
│        PAILLASSE (canevas)                        │ Inspecteur    │
│   carte + breadboard + composants + câbles        │ PA5  D13      │
│   clic sur un bouton, rotation d'un potar,        │ Sortie PP     │
│   curseur de température…                         │ État : HAUT   │
├───────────────────────────────────────────────────┴───────────────┤
│ Série │ Traceur │ Analyseur │ Bus │ Réseau │ Fichiers │ Build │ ⚠ │
│ [UART2 115200] > Boot OK                                          │
└───────────────────────────────────────────────────────────────────┘
```

Un seul écran : canevas au centre, panneaux en onglets en bas, inspecteur à droite, tout est repliable.

### 10.4 Fonctions de la paillasse virtuelle

| Sur une vraie paillasse | Dans l'app | Priorité |
| --- | --- | --- |
| Carte + breadboard + câbles | Canevas, mode Édition (placer, câbler) et mode Simulation (interagir) | V1 |
| Doigts, potards, environnement | Clic, glisser, curseurs ; panneau environnement | V1 |
| Terminal série | Moniteur série : un onglet par UART, envoi de texte, fin de ligne, horodatage, vue hexa, export, alerte de débit | V1 |
| Traceur série Arduino | Traceur : courbes depuis `temp:21.4,hum:55` | V1 |
| Bouton reset, interrupteur d'alim | Reset (redémarre le firmware), coupure (RAM perdue, flash conservée) | V1 |
| Multimètre / sonde | Inspecteur de broche : mode, fonction, état, rapport cyclique, valeur ADC | V1 |
| Liste d'avertissements | Onglet ⚠ (section 9.2) | V1 |
| Indicateur d'alimentation de labo | Indicateur CPU / RAM par carte dans la barre d'état | V1 |
| Analyseur logique | Chronogramme des GPIO + capture | V2 |
| Décodeur de protocole | Onglet Bus : I2C, SPI, UART décodés (adresse, registre, données, ACK/NACK) | V2 |
| Oscilloscope | Courbe PWM, ADC, DAC dans le temps | V2 |
| Vue brochage CubeMX | Carte avec les fonctions configurées par le code | V2 |
| Wireshark, client MQTT | Onglet Réseau : connexions et messages du shim Wi-Fi | V2 |
| Lecteur de carte SD | Onglet Fichiers : flash simulée (LittleFS, SPIFFS, NVS) et carte SD, lecture, modification, effacement | V2 |

### 10.5 Au-delà d'une vraie paillasse

- **Scénarios de test** (fichiers YAML) : « à t = 2 s, appuyer sur B1 ; à t = 5 s, température 40 °C ; vérifier que la LED rouge s'allume ». V2.
- **CLI sans interface** : `prebench run`, `prebench test scenarios/*.yaml`, utilisable depuis tout éditeur et en CI GitHub. V2.
- **Enregistrement et rejeu** d'une session (GPIO, trames, série). V2.
- **Export pour le vrai matériel** : liste de câblage (« OLED SDA → PB9 / D14 »), nomenclature, `pins.h` optionnel. V1 pour la liste de câblage.

### 10.6 Ergonomie

- Palette de commandes `Cmd/Ctrl+K` (ajouter un composant, changer de carte).
- Raccourcis : `Espace` pause, `R` reset, `E` mode édition ; touches du clavier associables aux boutons de la paillasse (`1` = B1).
- Câblage assisté : en tirant un fil depuis une broche de composant, seules les broches compatibles de la carte s'allument.
- Extension VS Code (commandes, état, sortie série) : Plus tard, non nécessaire grâce à la surveillance du dossier, aux liens cliquables et à la CLI.

### 10.7 Réglages

**Principe.** Toute valeur réglable est déclarée à un seul endroit (`app/src-tauri/src/settings.rs`), avec sa portée et sa valeur par défaut ; le cœur lit ces défauts, l'interface reçoit la liste (`get_settings`). Deux niveaux :

- **app** : préférences de l'utilisateur, dans `settings.toml` du dossier de configuration de l'app (`~/Library/Application Support/dev.prebench.app`, `%APPDATA%\dev.prebench.app`) ;
- **projet** : réglages propres à un montage, dans une table `[settings]` de `prebench.toml`, ajoutée au schéma (ajout compatible) avec le premier réglage de projet modifiable. Une valeur du projet l'emporte sur celle de l'app.

Chaque brique arrive quand elle devient utile : un réglage peut être déclaré « fixe » (sa valeur par défaut fait foi, rien ne la modifie encore), puis devenir modifiable. Un fichier modifié à la main avec une valeur invalide est ignoré : la valeur par défaut s'applique.

| Réglage | Portée | Défaut | État |
| --- | --- | --- | --- |
| Thème (`system`, `light`, `dark`) | app | `system` | Modifiable (bouton de la barre d'outils) |
| Éditeur pour ouvrir `fichier:ligne` | app | VS Code | Fixe ; autres éditeurs : Plus tard |
| Images par seconde, sur secteur / sur batterie | app | 60 / 30 | Fixe |
| Pause automatique : CPU du firmware et durée | projet | 80 % d'un cœur, 5 s | Fixe |
| Plafond mémoire du firmware | projet | 256 Mo | Fixe ; modifiable : Plus tard |
| Simulation quand la fenêtre est masquée | app | continue | Prévu |
| Accès internet du shim Wi-Fi | projet | non (localhost) | Prévu (déjà `network.internet` dans le schéma, V2) |

**Écran des réglages** : Plus tard ; en attendant, seuls les contrôles existants (bouton de thème) modifient un réglage.

## 11. Performance, ressources et sécurité

Aucun bug du firmware ne doit pouvoir geler l'ordinateur : au pire, la simulation se met en pause avec un message clair.

### 11.1 Budgets cibles

| Situation | CPU | RAM |
| --- | --- | --- |
| App ouverte, pas de simulation | < 1 % | < 150 Mo |
| 1 Nucleo + OLED + 5 composants simples | < 10 % d'un cœur | < 250 Mo |
| ESP32 + TFT + audio I2S | < 25 % d'un cœur | < 400 Mo |
| Plafond dur par carte | Pause automatique au-delà de 80 % d'un cœur pendant 5 s | 256 Mo par firmware |

Taille de l'app visée : de l'ordre de 10 à 20 Mo, hors toolchains.

### 11.2 Garde-fous système

| Garde-fou | macOS | Windows | Défaut |
| --- | --- | --- | --- |
| Plafond mémoire par firmware | Surveillance de l'empreinte mémoire (`proc_pid_rusage`) à 1 Hz, à 4 Hz au-delà de la moitié du plafond, et arrêt au-delà du plafond : macOS n'applique pas `setrlimit` (`RLIMIT_AS`, `RLIMIT_DATA`) | Même surveillance + Job Object (plafond système au double) ; Linux : même surveillance + `RLIMIT_AS` au double | 256 Mo surveillés, arrêt du firmware + bannière ; limite réglable dans `prebench.toml` : Plus tard ; plafond système sous macOS par `taskpolicy -m <Mo>` au lancement (option à évaluer) : Plus tard |
| Surveillance CPU par enfant | `proc_pid_rusage` | `GetProcessTimes` (windows-sys, sans crate supplémentaire) | Minuteur à 1 Hz, actif seulement pendant une simulation et hors pause ; 5 s de temps CPU consommé à plus de 80 % d'un cœur (temps CPU du processus, pas durée écoulée : indépendant de la charge de la machine) → pause (`PAUSE`, puis suspension du processus) + bannière « Reprendre » |
| Cœurs économes | QoS « utility » (cœurs E) | EcoQoS (Windows 11) | Activé |
| Cartes simultanées | Limite de l'app | Limite de l'app | 4 |
| Plantage firmware | Processus séparé, crash et pile d'appels affichés | Idem |  |
| Processus orphelins | libvmcu quitte à la fermeture du socket ; groupe de processus tué à la sortie | Job Object « kill on close » | Zéro processus fantôme |
| Tampons | Circulaires, taille fixe | Idem | Moniteur série 1 Mo, analyseur N échantillons |

### 11.3 Rendu de l'interface

- **Coalescence** : le cœur envoie à l'interface l'état par image (`requestAnimationFrame`), jamais chaque événement.
- Luminosité d'une LED en PWM = rapport cyclique mesuré sur l'image.
- Écrans : redessinés uniquement si le framebuffer est marqué « modifié ».
- Fenêtre masquée ou réduite : rendu arrêté, la simulation continue.
- Sur batterie : 30 images/s au lieu de 60.
- Mode temps virtuel (plus tard) : plafonné à ×1 par défaut, accélération sur demande.

### 11.4 Sécurité

- **Fichiers** : les shims de fichiers (NVS, SD, LittleFS, SPIFFS) n'écrivent que dans le dossier du projet.
- **Réseau** : le shim Wi-Fi n'autorise que `localhost` par défaut ; l'accès à internet est un interrupteur explicite par projet.
- **Modèle de confiance** : ouvrir un projet, c'est compiler et exécuter son firmware en code natif ; **un projet est donc du code de confiance**, au même titre qu'un projet qu'on compile soi-même. Seuls les **modèles** de composants sont isolés (QuickJS : pas de disque, de réseau ni de système, limites mémoire et temps). Le **firmware** (processus natif, protégé seulement par les garde-fous de ressources de la section 11.2) et les **vues** (code exécuté dans la WebView) ne sont **pas** isolés.
- **Avertissement à l'ouverture d'un projet d'origine inconnue** (Plus tard) : prévenir avant de compiler et d'exécuter un projet téléchargé ou reçu.
- **Tauri capabilities** : l'interface ne peut appeler qu'une liste stricte de commandes du cœur.
- **Distribution** : versions publiées sur GitHub, signature et mises à jour décrites en section 12.5 ; aucun secret dans le code publié.

## 12. Hors ligne, multiplateforme et distribution

L'app fonctionne entièrement hors ligne et cible macOS (Apple Silicon) et Windows 10/11 dès le départ ; Linux est presque gratuit avec cette structure.

### 12.1 Hors ligne

- App, simulation, composants, cartes et compilation : tout est local.
- Seul cas nécessitant internet : un firmware qui veut joindre un vrai serveur en ligne, et seulement si l'interrupteur internet du projet est activé. Alternative recommandée : un broker local (Mosquitto).

### 12.2 Ce qui diffère selon l'OS

| Brique | macOS | Windows | Solution commune |
| --- | --- | --- | --- |
| Socket firmware ↔ app | Socket Unix | Named pipe | Crate `interprocess` |
| Mémoire partagée | `mmap` | `CreateFileMapping` | Crate `shared_memory` |
| Plafonds mémoire / CPU | `setrlimit` + surveillance | Job Objects (natifs) | Module `platform/` |
| Cœurs économes | QoS utility | EcoQoS | Module `platform/` |
| Mesure CPU / RAM | `proc_pid_rusage` | API Windows | Crate `sysinfo` |
| Audio, caméra | CoreAudio, AVFoundation | WASAPI, Media Foundation | Crates `cpal`, `nokhwa` dans le cœur |
| Sockets du shim Wi-Fi | POSIX | Winsock | Couche d'adaptation dans libvmcu |
| Moteur web | WebKit | Chromium (WebView2) | Pas de fonctions CSS/JS trop récentes |

### 12.3 Pièges connus

1. **Compilation hôte sous Windows** : le code HAL et ESP-IDF utilise des extensions GCC (`__attribute__((packed))`…). Utiliser **clang ou MinGW-w64, jamais MSVC**. Fichiers de toolchain CMake fournis par OS pour que le build « sim » soit identique partout. Cette interdiction vise le **firmware** de l'utilisateur et libvmcu. L'app Rust, elle, est compilée avec la cible MSVC (`x86_64-pc-windows-msvc`), cible standard de Tauri sous Windows.
2. **Précision des délais** sous Windows : timers haute résolution obligatoires (section 5.5).
3. **FreeRTOS** : port POSIX sur Mac, port Windows sous Windows ; même suite de tests sur les deux.
4. **Chemins et fins de ligne** : normalisés dans le cœur Rust ; LF forcé dans les fichiers de projet.
5. **Signature** : une app non signée déclenche Gatekeeper (macOS) et SmartScreen (Windows) chez les utilisateurs ; procédure documentée en section 12.5.

### 12.4 Méthode

- Développement sur Mac, dans le dépôt privé.
- CI du dépôt privé : Windows et Linux à chaque push. Les minutes macOS y sont décomptées dix fois sur le quota gratuit : les tests macOS se font en local.
- CI complète macOS + Windows dans le dépôt public, à chaque release, gratuite et illimitée pour un dépôt public.
- Surcoût estimé du multiplateforme prévu dès le départ : environ 20 à 30 % d'effort, surtout sur la compilation hôte et les délais.

### 12.5 Distribution et releases

Prebench est open source et téléchargeable librement ; le dépôt public ne reçoit qu'un commit par version, l'historique de travail reste privé (D13).

| Élément | Choix |
| --- | --- |
| Dépôt privé | `prebench-dev` : tout le travail quotidien, historique complet |
| Dépôt public | `prebench` : un commit « Release vX.Y.Z » par version, README, CHANGELOG, licences, issues, GitHub Releases |
| Publication | `release.sh vX.Y.Z` : `git archive` du tag, exclusions `.gitattributes export-ignore`, commit unique + tag dans le clone public, push en HTTPS par défaut (`https://github.com/rdkwcz08/prebench.git`), SSH possible via la variable `PREBENCH_PUBLIC_REMOTE` |
| Build de release | GitHub Actions du dépôt public : `.dmg` macOS arm64 et `.msi` Windows x64, publiés en GitHub Release **normale** (jamais en pré-release) |
| Vérification avant publication | Dans `release.yml`, l'app est testée telle qu'installée : `.msi` installé en silencieux dans `C:\Program Files\Prebench`, `Prebench.app` copiée depuis le `.dmg` dans `/Applications` en lecture seule. Lancée avec `--verifier-installation`, sans fenêtre, elle crée un projet depuis le modèle blinky, le compile avec le sdk installé en trouvant CMake et le compilateur hors du PATH, et attend que LD2 clignote ; sous Windows, rien ne doit être écrit dans le dossier d'installation. Un échec bloque la release |
| Mises à jour | Plugin updater de Tauri : lit `latest.json` à l'adresse `releases/latest/download/latest.json` du dépôt public (dernière release normale) et vérifie chaque mise à jour avec la clé de signature du projet |
| Versions | SemVer `0.x` jusqu'à la stabilité, tags `vX.Y.Z` uniquement (pas de suffixe `-rc`). Les versions `0.x` sont des releases normales : le numéro `0.x`, le README et la mention « aperçu technique » dans les notes de version signalent leur maturité. `CHANGELOG.md` rédigé pour les utilisateurs, binaires `Prebench_X.Y.Z_aarch64.dmg` et `Prebench_X.Y.Z_x64.msi` |
| Signature macOS | Pas de notarisation au départ : procédure « Ouvrir quand même » (Réglages Système → Confidentialité et sécurité) documentée dans le README ; notarisation Apple plus tard (programme développeur payant) |
| Signature Windows | Pas de certificat au départ : avertissement SmartScreen documenté (« Informations complémentaires → Exécuter quand même ») |
| Contributions externes | Pull requests sur le dépôt public, reportées à la main dans le dépôt privé (`git format-patch` / `git am`) |

**Règles de publication**

- Aucun secret dans l'arborescence exportée : clé de l'updater et tokens uniquement dans les secrets GitHub.
- Fichiers internes (notes, brouillons) marqués `export-ignore` ; ce qui est publié l'est définitivement.
- libvmcu, shims, `boards/` et `components/` sous licence MIT comme le reste du projet (D16) : les utilisateurs les compilent avec leur propre code.

## 13. Extensions prévues et limites assumées

Onze types de projets dépassent la version de base : huit deviennent des ajouts sans refonte grâce aux trois créneaux d'avenir (D12), trois restent volontairement hors périmètre.

| # | Projet type | Ce qui bloque | Ce qu'il faut ajouter | Verdict |
| --- | --- | --- | --- | --- |
| E1 | IoT Wi-Fi : station météo MQTT, serveur web ESP32 | `esp_wifi`, lwIP, `WiFiClient`absents | Shim qui « connecte » instantanément et passe par les sockets de l'hôte ; panneau coupure Wi-Fi / RSSI | À faire, priorité n°1 |
| E2 | Plusieurs cartes : UART croisé, CAN, passerelle | Un seul firmware à la fois | Plusieurs processus sur le même réseau logique ; composant bus CAN (`HAL_CAN_*`, TWAI) | À faire |
| E3 | Projet qui déborde la RAM de la vraie carte | L'hôte a des Go de RAM | Build de la vraie cible à chaque compilation + jauge RAM/flash via `size` ; tas optionnellement limité | À faire (V1) |
| E4 | Audio : enceinte connectée, MP3, mot-clé | I2S = flux continu | Shim I2S vers les haut-parleurs et le micro de l'hôte via le canal flux | À faire si projet audio |
| E5 | BLE : capteur + app smartphone | Pas de pile Bluetooth | Shim GATT + panneau « téléphone virtuel » (pas de vrai BLE) | Plus tard (V2+) |
| E6 | Asservissement : suiveur de ligne, pendule, PID | Pas de monde physique, gigue de l'hôte | Composants à modèle physique + mode temps virtuel à pas fixe | Plus tard (second produit) |
| E7 | Basse consommation, deep sleep | API de veille absentes | États de veille (deep sleep = redémarrage), sources de réveil | États oui, estimation batterie non |
| E8 | ESP32-CAM, clavier HID, CDC | Caméra et USB absents | `esp_camera` via webcam ; TinyUSB vers un panneau | À la demande |
| E9 | Temps réel dur : VGA, fréquencemètre, FOC | Exige un émulateur au cycle près | — | Hors périmètre (Renode ponctuel) |
| E10 | Analogique : filtres AOP, conditionnement | Exige SPICE | Composant pont diviseur simplifié seulement | Hors périmètre (LTspice) |
| E11 | Bootloader, OTA, flash, registres, LL | Contraire au principe du shim | API `esp_ota_*` vers fichier, au mieux | Hors périmètre |

Le mode temps virtuel (E6) est rendu possible par la règle « tout le temps passe par libvmcu » : `vmcu_millis()` et `vmcu_delay_ms()` basculeront sur une horloge simulée sans modifier les shims.

## 14. Feuille de route

Six phases séquentielles ; chacune se termine par un critère de passage vérifiable avant d'ouvrir la suivante.

### Phase 0 — Contrats (avant tout code)

- Ajouter le fichier LICENSE MIT (D16)
- Figer l'API libvmcu (`libvmcu/include/vmcu.h`)
- Figer le protocole v1 (trames à en-tête fixe, D15) et l'écrire dans `docs/protocol.md`
- Figer `manifest.json`, l'interface `model.ts`, le format des cartes et `prebench.toml`
- Créer `prebench-dev` (privé) et `prebench` (public), `.gitattributes`, `release.sh`, CI Windows + Linux en privé et workflow de release en public
- Ajouter `docs/cahier-des-charges.md` et `CLAUDE.md`

**Critère de passage** : les quatre contrats sont écrits et versionnés et les deux dépôts créés.

### Phase 1 — Prototype (≈ 1 week-end)

- Shim `HAL_GPIO_*` + `HAL_Delay` / `HAL_GetTick` + `HAL_GPIO_EXTI_Callback`
- Socket firmware ↔ cœur Rust (`vmcu_platform_posix.c`, `vmcu_platform_win32.c`)
- Fenêtre Tauri avec une LED (LD2) et un bouton (B1)
- Recherche des outils de build (CMake, compilateur, make) dans le PATH, puis aux emplacements standard de chaque système, car une app lancée depuis le Finder ou installée par le `.msi` n'hérite pas du PATH du terminal ; chemins utilisés écrits dans le Journal ; vérification des prérequis au démarrage, avec pour chaque outil absent la commande d'installation et un renvoi au README
- Modèle `templates/blinky-stm32-hal/` avec cible « sim » CMake (clang macOS, MinGW/clang Windows)
- Projet de test `tests/cubemx-nucleo-f401re/` : projet STM32CubeMX de la Nucleo-F401RE en configuration par défaut (chaîne CMake), non modifié, compilé par la cible « sim » ; les modules HAL qu'il utilise sans simulation (USART2) compilent et avertissent « non simulé »
- Outil de mesure de la latence par `vmcu_ping` (messages `PING` / `PONG`), deux mesures :
  1. aller-retour firmware ↔ cœur Rust (`VMCU_PING_CORE`) : coût du socket et du routage ;
  2. aller-retour firmware ↔ cœur ↔ modèle de composant (`VMCU_PING_WORKER`), via un composant d'écho (`components/echo/`) placé sur la paillasse. Mesuré d'abord avec les modèles dans des Web Workers, puis dans QuickJS (D5).

**Préparation de la release v0.1.0** (avant la fin de la phase 1, section 12.5) :

1. Vérifier la disponibilité du nom `prebench` sur GitHub, puis créer le dépôt public `prebench`, vide (sans README ni licence).
2. Générer la paire de clés de l'updater (`npx tauri signer generate`) ; conserver la clé privée et son mot de passe hors du dépôt, avec une sauvegarde. Vérifier aussitôt le mot de passe en signant un fichier de test (`npx tauri signer sign`) : un mot de passe perdu impose une nouvelle clé, que les versions déjà installées refusent.
3. Inscrire la clé publique et l'adresse de `latest.json` dans `app/src-tauri/tauri.conf.json` (`plugins.updater`), avec `bundle.createUpdaterArtifacts = true`.
4. Enregistrer la clé privée et son mot de passe dans les secrets du dépôt public (`TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`).
5. Ajouter la section `## [0.1.0]` au `CHANGELOG.md`, avec la mention « aperçu technique », et la version `0.1.0` dans `tauri.conf.json`.

**Critère de passage** (validé le 12 novembre 2026) :

- [x] Un blinky STM32 HAL non modifié fait clignoter la LED, et le bouton déclenche `HAL_GPIO_EXTI_Callback`, sur Mac et Windows (test `blinky_led_and_button` en local sur Mac et en CI Windows et Linux ; app installée vérifiée par `release.yml` sur macOS et Windows).
- [x] Le projet `tests/cubemx-nucleo-f401re/` généré par STM32CubeMX (6.18.1, FW_F4 V1.28.3, chaîne CMake) compile tel quel pour la cible « sim » et s'exécute sans `Error_Handler`, sur Mac et Windows : `CMakeLists.txt` généré reconnu, `system_stm32f4xx.c`, `syscalls.c` et `sysmem.c` exclus, startup et `.ld` ignorés (tests `test_cubemx` et `cubemx_generated_project`, en local sur Mac et en CI Windows et Linux).
- [x] Les deux latences d'aller-retour (cœur seul, puis cœur + modèle via le composant d'écho) sont mesurées sur Mac et Windows et rapportées (médiane, 99e centile), et la question de la section 8.1 est tranchée (D5 : modèles dans QuickJS).
- [x] La chaîne de publication est validée de bout en bout par deux releases GitHub normales : v0.1.0 (`release.sh --dry-run` propre, commit et tag dans le dépôt public `prebench`, `release.yml` vert, `.dmg` et `.msi` installés et lancés sur Mac et Windows), puis v0.1.1 avec un changement minime, que l'app v0.1.0 installée propose et applique par son updater, sur Mac et Windows.

### Phase 2 — Socle V1

Étapes livrables, dans l'ordre. Chacune est testée, committée et passe la CI (Windows + Linux) avant la suivante. Toute étape qui touche l'interface applique la charte `docs/design.md` et la maquette `docs/design/maquette-ecran-principal.html`. Tests manuels sous Windows à chaque release seulement : entre deux releases, la CI suffit.

1. **CI et reports de la phase 1** : tests des vues dans un navigateur headless (Playwright), uniquement dans la CI Linux, avec montage réel des vues compilées à l'exécution (le test Node de la phase 1 ne vérifie que l'évaluation en module ES) ; captures de l'écran principal en thème clair et sombre publiées comme artefact de la CI ; rapport de `--verifier-installation` affiché une seule fois sur macOS.
2. **Fondations visuelles** (étape 1 bis) : jetons CSS `--pb-*` clair et sombre (charte, section 2), thème du système ou choix manuel, squelette de l'écran (charte, section 4), composants de base (charte, section 5), jeu d'icônes au trait ; vues led, button, echo et bannière des prérequis sur ces fondations.
3. **Garde-fous ressources** (étape 2, section 11.2) : plafond mémoire, cœurs économes, surveillance CPU à 1 Hz active uniquement pendant une simulation (0 % au repos), pause automatique annoncée par une bannière avec « Reprendre », sans dialogue bloquant ; indicateur CPU / RAM dans la barre d'état.
4. **Cycle édition → simulation** (étape 3, section 10.2) : surveillance du dossier, build « sim », erreurs `fichier:ligne` cliquables dans le panneau du bas, rechargement à chaud.
5. **Débogage** (étape 3 bis, section 10.2) : génération du `launch.json` (lldb sur Mac, gdb sous Windows), paillasse figée quand le débogueur s'arrête sur un point d'arrêt. → **release v0.1.2**
6. **Canevas** (étape 4) : tapis ESD, carte à plat, modes Édition / Simulation en contrôle segmenté, câblage assisté, fils colorés selon la fonction (modifiables), câblage enregistré dans `prebench.toml`, coalescence du rendu par image (section 11.3).
7. **Réseau logique et onglet Avertissements** (étape 5, section 9) : états des nets doublés d'un mot ou d'une forme, pastille de comptage.
8. **Moniteur série (UART) et traceur** (étape 6) : shim `HAL_UART_*`, `printf`, onglets par UART, horodatage, hexa, export, alerte de débit, traceur `clé:valeur`. → **release v0.1.3**
9. **Inspecteur de broche, reset et coupure d'alimentation** (étape 7).
10. **PWM et ADC** (étape 8) : LED variable, servo, potentiomètre, buzzer, LED RGB.
11. **Critère de passage** (étape 9) : mesure des budgets de la section 11.1 sur Mac et Windows. → **release v0.2.0**

**Critère de passage** : budgets de la section 11.1 respectés ; un firmware en `while(1)` sans délai ne dépasse pas le plafond.

### Phase 3 — Bus et écrans

- I2C + OLED SSD1306 + MPU6050 + joker I2C
- SPI + TFT ILI9341 via mémoire partagée
- WS2812 via shim de bibliothèque
- Jauge mémoire de la vraie cible ; export de la liste de câblage
- Palette de commandes `Cmd/Ctrl+K` (section 10.6) et panneau environnement (section 8.5), reportés de la phase 2

**Critère de passage** : les dix composants du catalogue initial (8.4) fonctionnent.

### Phase 4 — ESP32 et Arduino

- Shim ESP-IDF (GPIO, LEDC, ADC, UART, I2C, SPI, NVS) + FreeRTOS POSIX / Windows
- Shim Arduino commun aux deux familles
- Fichier de carte ESP32 avec ses contraintes de broches

**Critère de passage** : le même projet tourne sur la carte Nucleo et sur l'ESP32 virtuelles.

### Phase 5 — V2

- Wi-Fi via les sockets de l'hôte (E1) et onglet Réseau
- Multi-cartes et CAN (E2)
- Audio I2S (E4), onglet Fichiers (SD, LittleFS, NVS)
- Analyseur logique, onglet Bus, oscilloscope, vue brochage
- Scénarios de test, CLI `prebench test`, enregistrement et rejeu
- Matrices, autres jokers, catalogue élargi

Ensuite, selon les besoins : BLE GATT, veille, temps virtuel et modèles physiques, caméra, USB, extension VS Code.

## 15. Questions ouvertes

Ces points ne sont pas encore tranchés.

- **Licence de wokwi-elements** : à vérifier avant réutilisation des visuels (MIT supposée).
- **Nom « prebench »** : disponibilité sur GitHub à vérifier avant de créer le dépôt public.
- **Notarisation Apple et certificat Windows** : à quel moment les financer.

## 16. Consignes pour une IA générative

Cette section se transmet avec le document ; elle fixe les règles que tout code produit DOIT respecter.

### 16.1 Contexte à donner

- Rôle : « Tu aides à développer Prebench, décrit dans ce cahier des charges. Les décisions de la section 2 ne se rediscutent pas. »
- Utilisateur : ingénieur en électronique embarquée, sur MacBook Apple Silicon et VS Code ; à l'aise en C embarqué et en HTML/CSS/JavaScript.
- Dans le dépôt : ce document est `docs/cahier-des-charges.md` ; `CLAUDE.md` à la racine résume cette section et y renvoie.
- Toujours préciser la phase (section 14) et la section concernée par la tâche.

### 16.2 Règles d'architecture

1. NE DOIT PAS simuler le cœur, l'horloge ou les registres d'un microcontrôleur (D1).
2. Tout appel d'un shim passe par l'API `vmcu_*` ; aucun shim ne parle directement au socket.
3. Tout le temps (millis, micros, délais) passe par libvmcu.
4. Chaque message du protocole porte l'identifiant de carte ; le chemin critique n'utilise jamais de JSON.
5. Les gros flux passent par la mémoire partagée, jamais par le socket.
6. Aucune API système hors de `platform/` (Rust) et `vmcu_platform_*.c` (C).
7. Aucun composant codé en dur dans le cœur : tout composant est un dossier `components/<id>/`.
8. Les modèles de composants s'exécutent dans QuickJS, dans le cœur, et n'ont accès ni au disque, ni au réseau, ni au système ; firmware et vues ne sont pas isolés (un projet est du code de confiance, 11.4).
9. Le cœur Rust ne poll jamais : tout est événementiel.
10. Une fonction de framework non simulée compile et avertit, ne provoque jamais d'erreur de lien.
11. Tout tampon est borné.

### 16.3 Qualité attendue

- Code prêt à copier-coller, complet, avec les chemins de fichiers en tête.
- Architecture propre et minimale : pas de dépendance ajoutée sans justification.
- Compatible macOS et Windows ; signaler toute différence de comportement.
- C : C11, compilable avec clang et MinGW-w64, sans extension MSVC.
- Rust : `cargo clippy` sans avertissement ; erreurs typées, pas de `unwrap()` dans le cœur.
- TypeScript : mode strict.
- Tests : chaque shim est accompagné d'un firmware d'exemple qui l'exerce.
- Commentaires et messages de l'interface en français ; identifiants de code en anglais.

### 16.4 Modèle de demande

```
Contexte : cahier des charges Prebench ci-joint.
Phase : 3 — Bus et écrans. Sections concernées : 6, 8.
Tâche : implémenter le composant SSD1306 (manifest.json, model.ts, view.svelte).
Contraintes : interface model.ts v1, rendu uniquement si l'état change.
Livrable : fichiers complets + firmware de test STM32 HAL.
```

### 16.5 Prompt de démarrage (phases 0 et 1)

Les décisions étant dans ce document, le prompt se limite à la mission.

```
# Contexte
Tu vas m'aider à développer Prebench. Le cahier des charges complet est dans
docs/cahier-des-charges.md : lis-le en entier avant de répondre, en particulier
les sections 1, 2 et 16. Les décisions de la section 2 et les règles des sections
16.2 et 16.3 ne se rediscutent pas ; si une demande les contredit, signale-le.

# Mission, étape 1 : phase 0 « Contrats » (section 14), sans code applicatif
Produis, complets et précédés de leur chemin :
1. Le squelette du dépôt (section 4.3) : README.md, LICENSE (MIT), .gitignore,
   .gitattributes (export-ignore), CHANGELOG.md.
2. docs/protocol.md v1 : en-tête de trame (champs, tailles, little-endian), poignée
   de main et versionnement, table des messages avec identifiant numérique et charge
   utile, regroupement par lots, transactions synchrones, canal flux.
3. libvmcu/include/vmcu.h : API vmcu_* complète, commentée, C11, codes d'erreur.
4. components/component-api.d.ts, components/manifest.schema.json et un exemple
   complet components/led/.
5. boards/board.schema.json et boards/nucleo-f401re.toml (LD2, B1, USART2 PA2/PA3,
   I2C1 PB8/PB9, SPI1, 4 entrées ADC, 4 sorties PWM).
6. Le schéma de prebench.toml.
7. CLAUDE.md : résumé de la section 16 et renvoi au cahier des charges.
8. release.sh, .github/workflows/ci.yml (privé : Windows + Linux) et release.yml
   (public : macOS arm64 + Windows x64, GitHub Release, updater Tauri), section 12.5.
Avant d'écrire, liste en 10 lignes maximum tes hypothèses et les points ambigus.
Après l'étape 1, ARRÊTE-TOI et attends ma validation.

# Étape 2 (après validation) : phase 1 « Prototype »
Réalise la liste de la phase 1 (section 14) jusqu'à son critère de passage,
mesure de latence comprise.

# Format de tes réponses
- Fichiers complets, prêts à copier-coller, chemin en tête.
- Commandes exactes pour compiler, lancer et vérifier ; ce que je dois tester et
  le résultat attendu.
- Différences de comportement macOS / Windows signalées.
- Aucune fonctionnalité absente du cahier des charges sans me le demander.
```

## 17. FAQ et glossaire

### 17.1 FAQ

**Pourquoi ne pas émuler le vrai binaire ?** Renode et QEMU le font, mais demandent d'embarquer un moteur lourd, couvrent mal certains périphériques et compliquent l'app. Pour valider la logique d'un projet avant réception des composants, le shim suffit (D1).

**Pourquoi ne pas utiliser Wokwi ?** Il fait une partie du travail, mais son extension VS Code exige internet sauf abonnement Pro, et le catalogue STM32 est réduit. Prebench vise le 100 % local, des cartes au choix et des composants à soi.

**Mon code tournera-t-il sans modification sur la vraie carte ?** Oui, si tu passes par le HAL ou le framework. Le code qui écrit dans les registres ne fonctionne pas en simulation.

**Comment ajouter une carte ?** Un fichier TOML dans `boards/` (section 7).

**Comment ajouter un composant ?** Un dossier `components/<id>/` avec trois fichiers (section 8). Pour une puce non modélisée, utiliser un joker.

**Comment simuler un WS2812 ou un DHT22 ?** Via le shim de la bibliothèque, pas au niveau du signal (section 5.4).

**Mon programme fait 100 % de CPU ?** libvmcu insère des micro-pauses ; au-delà de 80 % pendant 5 s, la simulation se met en pause (section 11).

**Faut-il internet ?** Non. Seul un firmware qui joint un vrai serveur en ligne en a besoin, et seulement si l'interrupteur internet du projet est activé.

**Puis-je coder dans un autre éditeur que VS Code ?** Oui : l'app surveille le dossier, quel que soit l'éditeur.

**Que garantit une simulation réussie ?** La logique. Le timing, l'analogique, la consommation et le comportement radio se valident sur la vraie carte.

**Où télécharger Prebench ?** Dans les GitHub Releases du dépôt public `prebench` (`.dmg` pour macOS, `.msi` pour Windows) ; l'app se met ensuite à jour seule (section 12.5).

**Pourquoi le dépôt public n'a-t-il qu'un commit par version ?** Le développement se fait dans un dépôt privé ; seul l'état de chaque version publiée est poussé en public (D13).

### 17.2 Glossaire

| Terme | Définition |
| --- | --- |
| Shim | Bibliothèque de remplacement qui expose les mêmes fonctions que le vrai framework et les redirige vers l'app |
| libvmcu | Bibliothèque C liée au firmware simulé ; API interne `vmcu_*` commune à tous les shims |
| Build « sim » | Compilation du firmware pour l'ordinateur hôte, liée à libvmcu |
| Net | Ensemble de broches reliées par des câbles, dont l'état logique est calculé |
| Joker | Composant générique configurable sans code (I2C, SPI, UART, source analogique, sonde) |
| Canal flux | Mémoire partagée en tampon circulaire pour les gros volumes (écrans, audio, caméra) |
| Coalescence | Regroupement des événements pour n'afficher qu'un état par image |
| Temps virtuel | Mode futur où libvmcu fournit une horloge simulée à pas fixe |
| Paillasse | Canevas de câblage de Prebench, où l'on place la carte et les composants |
| Prebench | Nom de travail de l'application |
| Release | Version publiée : un commit unique dans le dépôt public + binaires macOS et Windows en GitHub Release |