# Protocole firmware ↔ app — version 1.1

Ce document est le contrat entre libvmcu (côté firmware) et le cœur Rust (côté app). Il complète la section 6 du cahier des charges et suit les décisions D7, D12 et D15. L'API C correspondante est `libvmcu/include/vmcu.h`.

Les mots **DOIT**, **NE DOIT PAS** et **PEUT** ont le sens défini en section 0 du cahier des charges.

## 1. Vue d'ensemble

| Canal | Transport | Usage |
| --- | --- | --- |
| Messages | Socket Unix (macOS, Linux) / named pipe (Windows), crate `interprocess` | Messages courts, binaires, groupés par lots |
| Flux | Mémoire partagée, crate `shared_memory` (`mmap` / `CreateFileMapping`) | Framebuffers, audio, caméra, données SPI volumineuses |

- Une connexion par processus firmware, donc par carte simulée.
- Le JSON n'apparaît jamais sur ces canaux (règle 16.2.4).
- Toutes les valeurs multi-octets sont en **little-endian**. Aucun champ n'est aligné : chaque côté lit et écrit octet par octet (pas de copie de `struct` C).

### 1.1 Lancement du processus firmware

Le cœur crée le serveur du socket, puis lance l'exécutable firmware avec ces variables d'environnement :

| Variable | Exemple | Rôle |
| --- | --- | --- |
| `PREBENCH_SOCKET` | `/tmp/prebench-4242-1-3.sock` ou `\\.\pipe\prebench-4242-1-3` | Adresse du canal messages, à utiliser telle quelle (le cœur la change à chaque lancement du firmware) |
| `PREBENCH_BOARD_ID` | `1` | Identifiant d'instance de carte (1..65535) |
| `PREBENCH_DEBUG` | `1` | Facultative (1.1). Firmware lancé par le débogueur de VS Code (configuration de `.vscode/launch.json` écrite par l'app) : libvmcu active le fil de débogage (section 5.5) |

Si `PREBENCH_SOCKET` est absent, libvmcu écrit un message sur `stderr` et termine le processus avec le code 69 (`VMCU_EXIT_NO_APP`).

Différence macOS / Windows : un chemin de socket Unix est limité à 104 octets sur macOS. Le cœur DOIT choisir un chemin court dans le dossier temporaire de l'utilisateur.

### 1.2 Identifier une broche

| Famille | Nom dans `boards/*.toml` | `port` | `pin` |
| --- | --- | --- | --- |
| STM32 | `PA5`, `PC13` | lettre − `A` (A = 0, C = 2) | numéro (5, 13) |
| ESP32 | `GPIO5`, `GPIO34` | 0 | numéro GPIO |

Limites : `port` < 16, `pin` < 64. La valeur `0xFF` signifie « aucune broche ».

## 2. Trame

Chaque trame est un en-tête fixe de **24 octets**, suivi de `length` octets de charge utile.

| Décalage | Taille | Champ | Type | Description |
| --- | --- | --- | --- | --- |
| 0 | 2 | `magic` | u16 | `0x4250` : les octets `'P' 'B'` sur le fil |
| 2 | 2 | `type` | u16 | Type de message (section 4) |
| 4 | 2 | `board_id` | u16 | Identifiant d'instance de carte (D12). `0` est réservé : aucune trame valide ne le porte |
| 6 | 2 | `flags` | u16 | bit 0 `SYNC` : une réponse est attendue ; bit 1 `REPLY` : réponse à une requête `SYNC` ; autres bits à 0 |
| 8 | 4 | `seq` | u32 | Numéro de séquence de l'émetteur : il commence à 1 et augmente de 1 par trame. Une trame `REPLY` reprend le `seq` de la requête |
| 12 | 4 | `length` | u32 | Taille de la charge utile en octets, de 0 à 65 536 |
| 16 | 8 | `timestamp_us` | u64 | Côté firmware : `vmcu_micros()` à l'émission. Côté app : temps de simulation estimé, à titre informatif |

Types de champs utilisés dans les charges utiles :

- `u8`, `u16`, `u32`, `u64`, `i16` : entiers little-endian.
- `str` : `u16` longueur + octets UTF-8, sans zéro final, 1024 octets au plus.
- `bytes[n]` : `n` octets bruts ; `n` est donné par un champ précédent.

Règles de validité :

- Un `magic` incorrect, un `length` supérieur à 65 536 ou un `board_id` différent de celui de la poignée de main sont des **erreurs de protocole**. Le récepteur ferme la connexion ; le firmware se termine avec le code 72 (`VMCU_EXIT_PROTOCOL`).
- Une charge utile plus longue que prévu reste valide : les octets en trop sont ignorés (section 3.2).
- Un `type` inconnu est ignoré. Le récepteur le journalise une fois par type.

## 3. Poignée de main et versionnement

### 3.1 Séquence

```
firmware                                         app (cœur Rust)
   │ connexion à PREBENCH_SOCKET                     │
   │── HELLO (SYNC, seq=1) ─────────────────────────▶│ vérifie version, carte, board_id
   │◀──────────── GPIO_INPUT / ADC_VALUE (état initial)
   │◀──────────── WELCOME (REPLY, seq=1)             │
   │ vmcu_init() rend la main au firmware            │
```

En cas de refus, l'app répond `REJECT` (REPLY) puis ferme la connexion. libvmcu affiche la raison sur `stderr` et se termine avec le code 72.

Délai : si `WELCOME` ou `REJECT` n'arrive pas dans les 5 s, libvmcu se termine avec le code 72.

### 3.2 Versions

- Version actuelle : **1.1**, notée majeure.mineure.
- Changer la **majeure** casse la compatibilité : l'app refuse toute majeure différente de la sienne (`REJECT`, raison 1).
- Changer la **mineure** reste compatible. Seuls ces ajouts sont permis : nouveaux types de messages, nouveaux champs **en fin** de charge utile, nouveaux bits de `flags` qu'on peut ignorer sans risque.
- La version retenue pour la session est la plus petite des deux mineures ; l'app l'indique dans `WELCOME`. Aucun des deux côtés n'envoie de message plus récent que cette version.

## 4. Messages

Les identifiants `0x0001` à `0x7FFF` vont du firmware vers l'app ; `0x8000` à `0xFFFF` vont de l'app vers le firmware. Une réponse porte en général l'identifiant de la requête + `0x8000`.

### 4.1 Firmware → app

| ID | Nom | Flags | Charge utile |
| --- | --- | --- | --- |
| `0x0001` | `HELLO` | SYNC | `u8 proto_major`, `u8 proto_minor`, `u8 api_major`, `u8 api_minor`, `u32 pid`, `str board` (`nucleo-f401re`), `str framework` (`stm32-hal`), `str libvmcu_version` |
| `0x0010` | `GPIO_CONFIG` | — | `u8 port`, `u8 pin`, `u8 mode`, `u8 pull`, `u8 edge`, `u8 af` (`0xFF` = aucune) |
| `0x0011` | `GPIO_WRITE` | — | `u8 port`, `u8 pin`, `u8 level` (0/1) |
| `0x0020` | `PWM_CONFIG` | — | `u8 unit`, `u8 channel`, `u16 reserved`, `u32 freq_hz` (0 = arrêt) |
| `0x0021` | `PWM_DUTY` | — | `u8 unit`, `u8 channel`, `u16 duty` (0..65535 = 0..100 %) |
| `0x0022` | `PWM_ATTACH` | — | `u8 unit`, `u8 channel`, `u8 port`, `u8 pin` |
| `0x0030` | `ADC_CONFIG` | — | `u8 unit`, `u8 channel` |
| `0x0040` | `UART_CONFIG` | — | `u8 id`, `u8 reserved[3]`, `u32 baud` |
| `0x0041` | `UART_TX` | — | `u8 id`, `u8 reserved`, `u16 len` (≤ 4096), `bytes[len]` |
| `0x0050` | `I2C_CONFIG` | — | `u8 bus`, `u8 reserved[3]`, `u32 freq_hz` |
| `0x0051` | `I2C_XFER` | SYNC | `u8 bus`, `u8 addr` (7 bits), `u16 tx_len`, `u16 rx_len`, `u16 reserved`, `bytes[tx_len]` (tx_len, rx_len ≤ 4096) |
| `0x0060` | `SPI_CONFIG` | — | `u8 bus`, `u8 mode` (0..3), `u8 lsb_first`, `u8 reserved`, `u32 freq_hz` |
| `0x0061` | `SPI_XFER` | SYNC si `RX` | `u8 bus`, `u8 cs_port`, `u8 cs_pin`, `u8 xfer_flags`, `u32 len`, puis les données (section 4.3) |
| `0x0070` | `STREAM_OPEN` | SYNC | `u16 stream_id`, `u8 kind`, `u8 format`, `u32 capacity`, `u16 width`, `u16 height`, `u32 sample_rate`, `u8 channels`, `u8 bits`, `u16 reserved`, `str shm_name` |
| `0x0071` | `STREAM_CLOSE` | — | `u16 stream_id` |
| `0x0072` | `STREAM_NOTIFY` | — | `u16 stream_id`, `u16 reserved`, `u32 commit_seq` |
| `0x0080` | `WARN` | — | `u16 code`, `str text` |
| `0x0081` | `LOG` | — | `u8 level` (0 erreur … 3 debug), `u8 reserved`, `str text` |
| `0x0090` | `HEARTBEAT` | — | vide. Depuis 1.1, seulement avec `PREBENCH_DEBUG=1` : toutes les 200 ms, et aussitôt après un arrêt du processus (section 5.5) |
| `0x00F0` | `PING` | SYNC | `u8 target` (0 cœur, 1 modèle d'écho), `u8 reserved[3]` |

Valeurs de `mode` : 0 entrée, 1 sortie push-pull, 2 sortie drain ouvert, 3 alternative push-pull, 4 alternative drain ouvert, 5 analogique, 6 désactivée.
Valeurs de `pull` : 0 aucune, 1 pull-up, 2 pull-down.
Valeurs de `edge` : 0 aucun, 1 montant, 2 descendant, 3 les deux.

Codes de `WARN` :

| Code | Signification |
| --- | --- |
| 0 | Avertissement générique |
| 1 | Fonction du framework non simulée ; `text` contient son nom |
| 2 | Débordement du tampon de réception UART |
| 3 | Attente active détectée, micro-pause insérée |
| 4 | Broche ou périphérique absent de la carte |

### 4.2 App → firmware

| ID | Nom | Flags | Charge utile |
| --- | --- | --- | --- |
| `0x8001` | `WELCOME` | REPLY | `u8 proto_major`, `u8 proto_minor` (version retenue), `u8 adc_bits`, `u8 reserved`, `u32 sync_timeout_ms` (défaut : 1000 ; une heure en session de débogage) |
| `0x8002` | `REJECT` | REPLY | `u16 reason`, `str text` — raisons : 1 version, 2 carte inconnue, 3 `board_id` inattendu, 4 framework non supporté par la carte |
| `0x8010` | `GPIO_INPUT` | — | `u8 port`, `u8 pin`, `u8 level` (0/1), `u8 net_flags` (bit 0 flottant, bit 1 conflit) |
| `0x8030` | `ADC_VALUE` | — | `u8 unit`, `u8 channel`, `u16 raw` (selon `adc_bits`) |
| `0x8041` | `UART_RX` | — | `u8 id`, `u8 reserved`, `u16 len` (≤ 4096), `bytes[len]` |
| `0x8051` | `I2C_REPLY` | REPLY | `u8 status` (0 ACK, 1 NACK adresse, 2 NACK donnée, 3 erreur de bus), `u8 reserved`, `u16 rx_len`, `bytes[rx_len]` |
| `0x8061` | `SPI_REPLY` | REPLY | `u8 status` (0 OK, 3 erreur), `u8 reserved[3]`, `u32 len`, `bytes[len]` |
| `0x8070` | `STREAM_OPENED` | REPLY | `u16 stream_id`, `i16 status` (code `vmcu_status_t`) |
| `0x8072` | `STREAM_NOTIFY` | — | Même charge utile qu'en 4.1, pour les flux produits par l'app (`AUDIO_IN`, `CAMERA`) |
| `0x80E0` | `RESET` | — | vide |
| `0x80E1` | `PAUSE` | — | vide |
| `0x80E2` | `RESUME` | — | vide |
| `0x80E3` | `SHUTDOWN` | — | vide |
| `0x80F0` | `PONG` | REPLY | `u8 target`, `u8 status` (0 OK, 1 aucun composant d'écho), `u16 reserved`, `u32 handler_us` (temps passé dans le cœur ou le modèle, à titre informatif) |

`GPIO_INPUT` porte l'état du net vu par la broche ; c'est le cœur qui calcule le réseau logique (section 9 du cahier des charges). Un net flottant est lu à 0, avec le bit « flottant » levé.

### 4.3 Données de `SPI_XFER`

Bits de `xfer_flags` :

| Bit | Nom | Signification |
| --- | --- | --- |
| 0 | `RX` | Le firmware attend des données reçues : la trame est `SYNC` |
| 1 | `BULK` | Les données émises sont dans le flux bulk (mémoire partagée), pas dans la trame |
| 2 | `NO_TX` | Pas de données émises : le composant reçoit `len` octets `0xFF` |

Selon les bits, la suite de la charge utile est :

- `NO_TX` : rien.
- sans `BULK` : `bytes[len]`, avec `len` ≤ 4096.
- `BULK` : `u16 stream_id`, `u16 reserved`, `u64 position`. Les `len` octets commencent à `position` dans le tampon circulaire du flux bulk (section 6). Le cœur avance `read_pos` une fois les octets consommés.

Une transaction `BULK` est toujours une écriture seule ; `RX` et `BULK` sont exclusifs. Une réception plus grande que 4096 octets est découpée par libvmcu en plusieurs transactions `SYNC` consécutives. La ligne CS reste dans le même état entre elles.

`cs_port` / `cs_pin` = `0xFF` : le cœur cherche le composant dont l'entrée CS est active, d'après l'état des nets. Si aucun composant n'est sélectionné, les données émises sont perdues et la réponse contient des `0xFF`.

## 5. Lots, ordre et transactions synchrones

### 5.1 Regroupement par lots

Un **lot** est une suite de trames écrites en un seul appel système. Il n'existe pas de trame conteneur : le récepteur lit le flux et découpe les trames grâce à `length`.

libvmcu accumule les trames dans un tampon d'émission borné de 64 Kio. Elle vide ce tampon :

1. avant toute trame `SYNC`, qui part dans le même lot ;
2. quand le tampon est plein ;
3. à l'entrée d'un délai d'au moins 1 ms (`vmcu_delay_ms`, `vmcu_delay_us`) ; un délai plus court laisse le lot au point 4, pour que les consignes d'une boucle rapide se regroupent ;
4. au plus **1 ms** après la première trame non envoyée. Le fil E/S n'arme ce minuteur que si le tampon n'est pas vide : aucune activité au repos.

Une transaction de bus = un message : une écriture SPI entière, pas un octet (section 6.1 du cahier des charges).

PWM : seuls la fréquence (`PWM_CONFIG`) et le rapport cyclique (`PWM_DUTY`) transitent, jamais les fronts ; une PWM qui tourne n'émet rien. Tant qu'un `PWM_DUTY` attend dans le lot, une nouvelle consigne du même canal remplace sa charge utile au lieu d'ajouter une trame : le canal porte au plus une consigne par canal PWM et par lot, quelle que soit la cadence du firmware. Une consigne identique à la précédente n'est pas renvoyée.

ADC : le cœur envoie `ADC_VALUE` pour chaque canal de la carte pendant la poignée de main (avant `WELCOME`), puis à chaque changement de la valeur brute. Valeur brute = tension ÷ `adc_vref` × (2^`adc_bits` − 1), bornée (fichier de carte).

Côté app, le cœur regroupe de la même façon ses trames vers le firmware à chaque tour de sa boucle d'événements.

### 5.2 Ordre et contre-pression

- Les trames d'un même sens sont traitées **dans l'ordre d'émission**. Une requête `SYNC` est donc traitée après toutes les trames asynchrones qui la précèdent (un `GPIO_WRITE` de CS précède le `SPI_XFER`).
- Les données logiques ne sont jamais abandonnées en silence (section 6.3). Si le tampon d'émission est plein et que le socket n'accepte plus rien, le fil appelant attend : la contre-pression ralentit le firmware, sans perte.
- Le cœur ne transmet à l'interface que l'état utile, une fois par image (coalescence, section 11.3) ; c'est l'interface qui saute des images, jamais le cœur qui perd des messages.
- Le tampon de réception UART du firmware est borné (4 Kio par UART). S'il déborde, les octets sont perdus et un `WARN` code 2 est émis, comme un dépassement (overrun) sur une vraie carte.

### 5.3 Transactions synchrones

Messages concernés : `HELLO`, `I2C_XFER`, `SPI_XFER` avec `RX`, `STREAM_OPEN` et `PING`.

1. Le fil appelant envoie la requête avec `SYNC` et attend la trame `REPLY` portant le même `seq`.
2. Plusieurs fils (tâches FreeRTOS) PEUVENT avoir chacun une requête en cours ; le `seq` les distingue.
3. Les messages asynchrones de l'app (`GPIO_INPUT`, `UART_RX`…) continuent d'être traités pendant l'attente par le fil E/S.
4. Délai : `sync_timeout_ms` (reçu dans `WELCOME`), ou celui que précise l'appel. Ce délai ne s'écoule pas pendant une pause. Une fois dépassé, la fonction renvoie `VMCU_ERR_TIMEOUT` ; une réponse tardive est ignorée.
5. Sans composant à l'adresse demandée, l'app répond `I2C_REPLY` NACK adresse, comme un vrai bus.
6. Chemin d'un aller-retour : firmware → cœur Rust → fil « modèles » (QuickJS, modèle du composant) → cœur → firmware. La WebView n'est jamais sur ce chemin.
7. `PING` mesure ce chemin en deux temps (section 8.1 du cahier des charges) :
   - `target = 0` : le cœur répond `PONG` sans quitter Rust (coût du socket et du routage) ;
   - `target = 1` : le cœur appelle `onPing` du modèle d'un composant d'écho (`components/echo/`) et répond `PONG` quand le modèle a rendu la main. C'est le même chemin qu'une transaction I2C ou SPI. Sans composant d'écho sur la paillasse, `PONG` porte `status = 1` et `vmcu_ping` renvoie `VMCU_ERR_UNSUPPORTED`.

### 5.4 Commandes de contrôle

| Message | Effet dans libvmcu |
| --- | --- |
| `PAUSE` | Chaque fil est bloqué à son prochain appel `vmcu_*`. `vmcu_millis()` / `vmcu_micros()` et les délais de transaction sont gelés. Le cœur PEUT en plus suspendre le processus par son module `platform/`, pour une boucle qui n'appelle jamais libvmcu |
| `RESUME` | Libère les fils ; le temps repart sans saut |
| `RESET` | Vide le tampon, ferme le canal, termine le processus avec le code 71 ; le cœur relance le firmware avec le même `board_id`, sans toucher à la paillasse |
| `SHUTDOWN` | Vide le tampon, ferme le canal, termine le processus avec le code 0 |
| Fermeture du socket par l'app | Termine immédiatement le processus avec le code 70 : aucun processus orphelin (cahier des charges, section 5.5) |

### 5.5 Débogage (1.1)

Pour déboguer, l'app ne lance pas le firmware : elle écrit dans `.vscode/launch.json` du projet une configuration qui le lance sous le débogueur avec `PREBENCH_SOCKET` (adresse fixe du projet), `PREBENCH_BOARD_ID` et `PREBENCH_DEBUG=1`, puis elle attend sa connexion sans délai.

Un débogueur arrêté sur un point d'arrêt fige tous les fils du processus. Avec `PREBENCH_DEBUG=1` et une version retenue ≥ 1.1, libvmcu démarre un fil de débogage :

- il lit l'horloge toutes les 100 ms et envoie `HEARTBEAT` toutes les 200 ms ;
- chaque lecture de l'horloge, par n'importe quel fil, la compare à la précédente : un écart de plus de 250 ms signale un processus figé, et cet écart (moins 100 ms) est retiré du temps de simulation, une seule fois. Le premier fil réveillé après un point d'arrêt le constate avant d'utiliser l'heure : aucune échéance ne tombe à tort, `vmcu_millis()` repart sans saut (à 100 ms près) ;
- côté app, sans `HEARTBEAT` pendant 600 ms, le firmware est « arrêté sur un point d'arrêt » ; ni pause CPU, ni suspension, ni arrêt pour mémoire pendant une session de débogage ;
- `WELCOME` porte un `sync_timeout_ms` d'une heure : une transaction en cours pendant un arrêt n'expire pas.

À la fin d'une session (débogueur arrêté, reset), le canal se ferme et l'app attend la session suivante sur la même adresse.

## 6. Canal flux (mémoire partagée)

### 6.1 Création et nommage

- libvmcu crée le segment (`vmcu_stream_open`), puis l'annonce par `STREAM_OPEN`. Le cœur l'ouvre et répond `STREAM_OPENED`.
- `stream_id` est attribué par libvmcu, de 1 à 65535, unique dans la session.
- Nom du segment :
  - macOS / Linux : `/pb<pid>_<board_id>_<stream_id>`. Sur macOS, un nom de segment POSIX est limité à 31 caractères ; ce format reste en dessous.
  - Windows : `Local\prebench_<pid>_<board_id>_<stream_id>`.
- Capacité maximale : 16 Mio par flux.
- Le flux `BULK` (kind 5) est ouvert par libvmcu à la première écriture SPI de plus de 4096 octets. Capacité par défaut : 256 Kio, soit plus qu'une trame TFT 320×240 en 16 bits (≈ 150 Ko).

### 6.2 Disposition du segment

En-tête de 64 octets, puis la zone de données (alignée sur 64 octets).

| Décalage | Taille | Champ | Description |
| --- | --- | --- | --- |
| 0 | 4 | `magic` | `0x4D534250` (`'P' 'B' 'S' 'M'`) |
| 4 | 2 | `layout_version` | 1 |
| 6 | 1 | `kind` | 1 framebuffer, 2 audio sortie, 3 audio entrée, 4 caméra, 5 bulk |
| 7 | 1 | `format` | Pixels : 1 RGB565, 2 RGB888, 3 mono 1 bit ; 0 sinon |
| 8 | 4 | `capacity` | Taille de la zone de données, en octets |
| 12 | 2 | `width` | Framebuffer, caméra |
| 14 | 2 | `height` | Framebuffer, caméra |
| 16 | 4 | `sample_rate` | Audio |
| 20 | 1 | `channels` | Audio |
| 21 | 1 | `bits` | Audio |
| 22 | 2 | `reserved` | 0 |
| 24 | 8 | `write_pos` | Atomique, écrit par le producteur : position d'écriture qui ne revient jamais à zéro |
| 32 | 8 | `read_pos` | Atomique, écrit par le consommateur |
| 40 | 4 | `commit_seq` | Atomique, incrémenté par le producteur à chaque `commit` |
| 44 | 4 | `ack_seq` | Atomique : dernier `commit_seq` traité par le consommateur |
| 48 | 16 | `reserved` | 0 |
| 64 | `capacity` | données | |

### 6.3 Règles

- **Tampon circulaire** (audio, caméra, bulk) : un seul producteur et un seul consommateur. L'octet à la position `p` est à `data[p % capacity]`. Le producteur n'écrit jamais au-delà de `read_pos + capacity`. Il écrit d'abord les données, puis publie `write_pos` (store *release*) ; le consommateur lit `write_pos` (load *acquire*), puis les données.
- **Framebuffer** : la zone contient une image complète. Le producteur écrit, puis incrémente `commit_seq`. L'image est « modifiée » tant que `commit_seq ≠ ack_seq`. Le consommateur PEUT sauter des images (section 6.3 du cahier des charges).
- **Notification sans scrutation** : après un `commit`, le producteur envoie `STREAM_NOTIFY` seulement si le consommateur était à jour (`ack_seq == commit_seq − 1`). Une fois l'image traitée, le consommateur écrit `ack_seq`, puis relit `commit_seq` ; s'il a changé, il recommence. Aucune notification n'est perdue et aucune scrutation n'est nécessaire (règle 16.2.9).
- Les flux produits par l'app (`AUDIO_IN`, `CAMERA`) suivent les mêmes règles, rôles inversés ; la notification est la trame `0x8072`.

## 7. Historique

| Version | Date | Changements |
| --- | --- | --- |
| 1.0 | 2026-11-10 | Version initiale (phase 0) |
| 1.1 | 2026-11-13 | Débogage (phase 2) : variable `PREBENCH_DEBUG`, message `HEARTBEAT` (`0x0090`), fil de débogage de libvmcu (section 5.5). Ajouts compatibles |
