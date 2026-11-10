/*
 * libvmcu/include/vmcu.h — API publique de libvmcu, contrat v1.0
 *
 * libvmcu est la bibliothèque C liée au firmware compilé pour l'ordinateur
 * (build « sim »). Les shims (STM32 HAL, ESP-IDF, Arduino, bibliothèques
 * tierces) traduisent les appels de leur framework vers cette API ; aucun
 * shim ne parle directement au socket (cahier des charges, règle 16.2.2).
 *
 * Références : docs/cahier-des-charges.md sections 5 et 6, docs/protocol.md.
 *
 * Conventions
 * - C11, compilable avec clang et MinGW-w64, sans extension MSVC.
 * - Toutes les fonctions sont sûres entre fils d'exécution (tâches FreeRTOS
 *   du port POSIX ou Windows, fils créés par le firmware).
 * - Une broche est désignée par (port, pin) :
 *     STM32 : PA5 -> port 0, pin 5 ; PC13 -> port 2, pin 13 (port = lettre - 'A').
 *     ESP32 : GPIO5 -> port 0, pin 5.
 * - Tout le temps (millis, micros, délais) passe par libvmcu (D12, règle 16.2.3).
 * - Les fonctions qui renvoient vmcu_status_t renvoient VMCU_OK (0) en cas de
 *   succès et un code négatif sinon.
 *
 * Fils d'exécution internes de libvmcu
 * - Fil « E/S » : lit le socket, complète les transactions synchrones, met à
 *   jour les caches (entrées GPIO, valeurs ADC, réception UART).
 * - Fil « IRQ » : exécute les callbacks (fronts GPIO, réception UART), un à la
 *   fois, avec le verrou d'interruption tenu (voir vmcu_irq_lock). Un callback
 *   PEUT appeler toute fonction vmcu_*, y compris une transaction synchrone.
 *
 * Licence MIT — voir LICENSE.
 */
#ifndef VMCU_H
#define VMCU_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ------------------------------------------------------------------------- */
/* Versions                                                                  */
/* ------------------------------------------------------------------------- */

/** Version de cette API C. Majeure : rupture ; mineure : ajouts compatibles. */
#define VMCU_API_VERSION_MAJOR 1
#define VMCU_API_VERSION_MINOR 0

/** Version du protocole firmware <-> app implémentée (docs/protocol.md). */
#define VMCU_PROTOCOL_VERSION_MAJOR 1
#define VMCU_PROTOCOL_VERSION_MINOR 0

/* Vérification du format printf, sans extension MSVC. */
#if defined(__MINGW32__) && defined(__MINGW_PRINTF_FORMAT)
#define VMCU_PRINTF_LIKE(fmt_index, args_index) \
    __attribute__((format(__MINGW_PRINTF_FORMAT, fmt_index, args_index)))
#elif defined(__GNUC__) || defined(__clang__)
#define VMCU_PRINTF_LIKE(fmt_index, args_index) \
    __attribute__((format(printf, fmt_index, args_index)))
#else
#define VMCU_PRINTF_LIKE(fmt_index, args_index)
#endif

/* ------------------------------------------------------------------------- */
/* Codes d'erreur                                                            */
/* ------------------------------------------------------------------------- */

typedef enum {
    VMCU_OK                  =   0, /* Succès. */
    VMCU_ERR_NOT_INITIALIZED =  -1, /* vmcu_init n'a pas réussi. */
    VMCU_ERR_INVALID_ARG     =  -2, /* Argument hors limites (port, pin, longueur…). */
    VMCU_ERR_UNSUPPORTED     =  -3, /* Fonction absente de cette carte ou non simulée. */
    VMCU_ERR_NACK            =  -4, /* I2C : adresse ou donnée non acquittée. */
    VMCU_ERR_TIMEOUT         =  -5, /* Pas de réponse de l'app dans le délai. */
    VMCU_ERR_DISCONNECTED    =  -6, /* Socket fermé (le processus va se terminer). */
    VMCU_ERR_NO_MEMORY       =  -7, /* Allocation ou mémoire partagée impossible. */
    VMCU_ERR_BUFFER_FULL     =  -8, /* Tampon borné plein, données non acceptées. */
    VMCU_ERR_PROTOCOL        =  -9, /* Trame invalide ou inattendue. */
    VMCU_ERR_VERSION         = -10, /* Version de protocole refusée par l'app. */
    VMCU_ERR_BUSY            = -11  /* Ressource déjà utilisée (flux déjà ouvert…). */
} vmcu_status_t;

/** Message lisible (français) pour un code d'erreur. Jamais NULL. */
const char *vmcu_strerror(vmcu_status_t status);

/* ------------------------------------------------------------------------- */
/* Cycle de vie                                                              */
/* ------------------------------------------------------------------------- */

/** Identifiant d'instance de carte attribué par l'app (D12). 0 = réservé à l'app. */
typedef uint16_t vmcu_board_id_t;

/** Codes de sortie du processus firmware, interprétés par le cœur Rust. */
#define VMCU_EXIT_SHUTDOWN      0  /* Arrêt demandé par l'app ou fin de main(). */
#define VMCU_EXIT_NO_APP       69  /* PREBENCH_SOCKET absent : lancé hors de l'app. */
#define VMCU_EXIT_DISCONNECTED 70  /* Socket fermé par l'app : sortie immédiate. */
#define VMCU_EXIT_RESET        71  /* Reset demandé : le cœur relance le processus. */
#define VMCU_EXIT_PROTOCOL     72  /* Erreur de protocole ou version refusée. */

/**
 * Connecte le firmware à l'app et effectue la poignée de main (HELLO/WELCOME).
 *
 * @param board      Type de carte, nom du fichier boards/<board>.toml sans
 *                   extension (ex. "nucleo-f401re").
 * @param framework  "stm32-hal", "esp-idf" ou "arduino".
 *
 * Le chemin du socket et l'identifiant d'instance sont lus dans les variables
 * d'environnement PREBENCH_SOCKET et PREBENCH_BOARD_ID posées par l'app.
 * Idempotente : les appels suivants renvoient VMCU_OK sans effet. Les shims
 * l'appellent au plus tôt (ex. HAL_Init, ou premier appel vmcu_* ).
 * Si l'app est absente ou refuse la connexion, le processus se termine avec
 * VMCU_EXIT_NO_APP ou VMCU_EXIT_PROTOCOL après un message sur stderr.
 */
vmcu_status_t vmcu_init(const char *board, const char *framework);

/** Vide les tampons, ferme le canal et libère les flux. Le processus continue. */
void vmcu_shutdown(void);

/** Identifiant d'instance reçu à la poignée de main (0 avant vmcu_init). */
vmcu_board_id_t vmcu_board_id(void);

/* ------------------------------------------------------------------------- */
/* Temps — tout le temps passe par ici (D12)                                 */
/* ------------------------------------------------------------------------- */

/**
 * Millisecondes depuis vmcu_init, hors périodes de pause (repasse à 0 après
 * environ 49,7 jours, comme HAL_GetTick).
 */
uint32_t vmcu_millis(void);

/** Microsecondes depuis vmcu_init, hors périodes de pause. */
uint64_t vmcu_micros(void);

/**
 * Sommeil réel (section 5.5), jamais une attente active. Vide le lot de
 * messages en attente avant de dormir. Sous Windows : timer haute résolution
 * (CREATE_WAITABLE_TIMER_HIGH_RESOLUTION) ; sous macOS : nanosleep.
 */
void vmcu_delay_ms(uint32_t ms);

/**
 * Délai court. Au-dessous de la résolution du système (≈ 50 µs macOS,
 * ≈ 500 µs Windows), la précision n'est pas garantie : le timing précis est
 * hors périmètre (section 3.3).
 */
void vmcu_delay_us(uint32_t us);

/* ------------------------------------------------------------------------- */
/* Sections critiques (équivalent __disable_irq / taskENTER_CRITICAL)        */
/* ------------------------------------------------------------------------- */

/**
 * Empêche l'exécution des callbacks tant que le verrou est tenu. Récursif :
 * chaque vmcu_irq_lock doit être suivi d'un vmcu_irq_unlock. Les callbacks
 * s'exécutent eux-mêmes avec ce verrou tenu.
 */
void vmcu_irq_lock(void);
void vmcu_irq_unlock(void);

/* ------------------------------------------------------------------------- */
/* GPIO                                                                      */
/* ------------------------------------------------------------------------- */

#define VMCU_GPIO_MAX_PORTS 16u   /* port < 16 */
#define VMCU_GPIO_MAX_PINS  64u   /* pin  < 64 */
#define VMCU_PIN_NONE       0xFFu /* « aucune broche » (ex. CS SPI non précisé) */
#define VMCU_GPIO_AF_NONE   0xFFu /* aucune fonction alternative */

typedef enum {
    VMCU_GPIO_MODE_INPUT        = 0,
    VMCU_GPIO_MODE_OUTPUT_PP    = 1, /* sortie push-pull */
    VMCU_GPIO_MODE_OUTPUT_OD    = 2, /* sortie drain ouvert */
    VMCU_GPIO_MODE_ALTERNATE_PP = 3, /* fonction alternative push-pull (PWM, SPI…) */
    VMCU_GPIO_MODE_ALTERNATE_OD = 4, /* fonction alternative drain ouvert (I2C) */
    VMCU_GPIO_MODE_ANALOG       = 5, /* entrée ADC */
    VMCU_GPIO_MODE_DISABLED     = 6  /* broche désactivée (HAL_GPIO_DeInit) */
} vmcu_gpio_mode_t;

typedef enum {
    VMCU_GPIO_PULL_NONE = 0,
    VMCU_GPIO_PULL_UP   = 1,
    VMCU_GPIO_PULL_DOWN = 2
} vmcu_gpio_pull_t;

typedef enum {
    VMCU_LOW  = 0,
    VMCU_HIGH = 1
} vmcu_level_t;

typedef enum {
    VMCU_GPIO_EDGE_NONE    = 0,
    VMCU_GPIO_EDGE_RISING  = 1,
    VMCU_GPIO_EDGE_FALLING = 2,
    VMCU_GPIO_EDGE_BOTH    = 3
} vmcu_gpio_edge_t;

/**
 * Callback de front, appelé depuis le fil IRQ (verrou tenu).
 * @param edge  front effectivement détecté (RISING ou FALLING).
 */
typedef void (*vmcu_gpio_edge_cb)(uint8_t port, uint8_t pin,
                                  vmcu_gpio_edge_t edge, void *user);

/**
 * Configure une broche. Envoie GPIO_CONFIG à l'app (inspecteur, validations).
 * @param af  numéro de fonction alternative (STM32 : GPIO_AFx), ou
 *            VMCU_GPIO_AF_NONE si le mode n'est pas ALTERNATE_*.
 */
vmcu_status_t vmcu_gpio_config(uint8_t port, uint8_t pin, vmcu_gpio_mode_t mode,
                               vmcu_gpio_pull_t pull, uint8_t af);

/** Écrit une sortie. Message asynchrone, groupé dans le lot courant. */
vmcu_status_t vmcu_gpio_write(uint8_t port, uint8_t pin, vmcu_level_t level);

/**
 * Lit le niveau d'une broche depuis le cache local (aucun aller-retour).
 * Entrée : dernier niveau envoyé par l'app (GPIO_INPUT). Sortie : dernier
 * niveau écrit. Broche inconnue : VMCU_LOW. Les lectures répétées sans
 * changement déclenchent la micro-pause anti-attente-active (section 5.5).
 */
vmcu_level_t vmcu_gpio_read(uint8_t port, uint8_t pin);

/**
 * Enregistre un callback de front sur une broche d'entrée (EXTI, ISR GPIO).
 * cb == NULL ou edge == VMCU_GPIO_EDGE_NONE désactive. Réémet GPIO_CONFIG
 * avec le front choisi. Un seul callback par broche.
 */
vmcu_status_t vmcu_gpio_on_edge(uint8_t port, uint8_t pin, vmcu_gpio_edge_t edge,
                                vmcu_gpio_edge_cb cb, void *user);

/* ------------------------------------------------------------------------- */
/* PWM — on simule le rapport cyclique, pas le signal (section 8.2)          */
/* ------------------------------------------------------------------------- */

/** Rapport cyclique maximal (100 %). */
#define VMCU_PWM_DUTY_MAX 65535u

/**
 * Configure un canal PWM désigné par (unité, canal).
 * STM32 : unité = numéro du timer, canal = 1..4 (TIM2_CH1 -> 2, 1).
 * ESP32 : unité = timer LEDC, canal = canal LEDC.
 * freq_hz == 0 arrête la sortie (HAL_TIM_PWM_Stop, ledc_stop).
 */
vmcu_status_t vmcu_pwm_config(uint8_t unit, uint8_t channel, uint32_t freq_hz);

/** Rapport cyclique, 0..VMCU_PWM_DUTY_MAX. */
vmcu_status_t vmcu_pwm_set_duty(uint8_t unit, uint8_t channel, uint16_t duty);

/**
 * Associe explicitement un canal à une broche (ESP32 : ledc_channel_config).
 * Facultatif sur STM32 : le cœur déduit la broche des `caps` et de la fonction
 * alternative configurée.
 */
vmcu_status_t vmcu_pwm_attach(uint8_t unit, uint8_t channel, uint8_t port, uint8_t pin);

/* ------------------------------------------------------------------------- */
/* ADC                                                                       */
/* ------------------------------------------------------------------------- */

/** Déclare l'utilisation d'un canal (inspecteur, validations). STM32 : unité 1. */
vmcu_status_t vmcu_adc_config(uint8_t unit, uint8_t channel);

/**
 * Valeur brute du canal, à la résolution de la carte (adc_bits), lue depuis le
 * cache local mis à jour par ADC_VALUE. Aucun aller-retour.
 */
uint16_t vmcu_adc_read(uint8_t unit, uint8_t channel);

/** Résolution ADC de la carte en bits (reçue à la poignée de main). */
uint8_t vmcu_adc_bits(void);

/* ------------------------------------------------------------------------- */
/* UART                                                                      */
/* ------------------------------------------------------------------------- */

/** Taille du tampon de réception borné, par UART. */
#define VMCU_UART_RX_BUFFER_SIZE 4096u

/**
 * Callback de réception, appelé depuis le fil IRQ avec les octets reçus.
 * Quand un callback est enregistré, les octets lui sont remis et ne sont PAS
 * placés dans le tampon de réception.
 */
typedef void (*vmcu_uart_rx_cb)(uint8_t id, const uint8_t *data, size_t len, void *user);

/** id = numéro du périphérique (USART2 -> 2, UART_NUM_0 -> 0). */
vmcu_status_t vmcu_uart_config(uint8_t id, uint32_t baud);

/** Émission non bloquante (découpée en trames UART_TX de 4 Kio au plus). */
vmcu_status_t vmcu_uart_write(uint8_t id, const uint8_t *data, size_t len);

/**
 * Lit au plus `max` octets du tampon de réception, en attendant au plus
 * timeout_ms (0 = sans attente). Renvoie le nombre d'octets lus.
 * Débordement du tampon : octets perdus + avertissement (comme un overrun réel).
 */
size_t vmcu_uart_read(uint8_t id, uint8_t *buf, size_t max, uint32_t timeout_ms);

/** Nombre d'octets disponibles dans le tampon de réception. */
size_t vmcu_uart_available(uint8_t id);

/** Enregistre (cb != NULL) ou retire (cb == NULL) le callback de réception. */
vmcu_status_t vmcu_uart_on_rx(uint8_t id, vmcu_uart_rx_cb cb, void *user);

/* ------------------------------------------------------------------------- */
/* I2C — transaction synchrone (section 6.1)                                 */
/* ------------------------------------------------------------------------- */

#define VMCU_I2C_MAX_XFER 4096u /* octets max par sens et par transaction */

/** Déclare un bus maître. bus = numéro (I2C1 -> 1). */
vmcu_status_t vmcu_i2c_config(uint8_t bus, uint32_t freq_hz);

/**
 * Transaction maître : écrit tx[0..txlen), puis (si rxlen > 0) start répété et
 * lit rxlen octets dans rx. txlen == rxlen == 0 : simple sonde d'adresse.
 * Bloque jusqu'à la réponse du composant modélisé.
 *
 * @param addr7       adresse 7 bits (0x3C pour un SSD1306).
 * @param timeout_ms  0 = délai par défaut annoncé par l'app (WELCOME).
 * @return VMCU_OK (ACK), VMCU_ERR_NACK, VMCU_ERR_TIMEOUT, VMCU_ERR_INVALID_ARG.
 */
vmcu_status_t vmcu_i2c_xfer(uint8_t bus, uint8_t addr7,
                            const uint8_t *tx, size_t txlen,
                            uint8_t *rx, size_t rxlen,
                            uint32_t timeout_ms);

/* ------------------------------------------------------------------------- */
/* SPI                                                                       */
/* ------------------------------------------------------------------------- */

/** Au-delà de cette taille, les données émises passent par la mémoire partagée. */
#define VMCU_SPI_INLINE_MAX 4096u

typedef enum {
    VMCU_SPI_MODE_0 = 0, /* CPOL 0, CPHA 0 */
    VMCU_SPI_MODE_1 = 1,
    VMCU_SPI_MODE_2 = 2,
    VMCU_SPI_MODE_3 = 3
} vmcu_spi_mode_t;

/** Déclare un bus maître. bus = numéro (SPI1 -> 1). */
vmcu_status_t vmcu_spi_config(uint8_t bus, vmcu_spi_mode_t mode, uint32_t freq_hz,
                              bool lsb_first);

/**
 * Transaction maître de `len` octets (une écriture entière = un message).
 *
 * @param cs_port, cs_pin  broche de sélection, ou VMCU_PIN_NONE pour les deux :
 *                         le cœur retrouve alors le composant sélectionné
 *                         d'après l'état des GPIO (CS piloté par le firmware).
 * @param tx  données émises, ou NULL (le composant reçoit des 0xFF).
 * @param rx  tampon de réception, ou NULL : écriture seule, ASYNCHRONE
 *            (aucune attente). Avec rx, la transaction est SYNCHRONE.
 * @param timeout_ms  0 = délai par défaut.
 *
 * Écriture seule > VMCU_SPI_INLINE_MAX : données copiées dans le flux « bulk »
 * en mémoire partagée (règle 16.2.5) ; l'appel attend s'il manque de place.
 */
vmcu_status_t vmcu_spi_xfer(uint8_t bus, uint8_t cs_port, uint8_t cs_pin,
                            const uint8_t *tx, uint8_t *rx, size_t len,
                            uint32_t timeout_ms);

/* ------------------------------------------------------------------------- */
/* Canal flux — mémoire partagée (D12, section 6.2)                          */
/* ------------------------------------------------------------------------- */

typedef enum {
    VMCU_STREAM_FRAMEBUFFER = 1, /* image complète + indicateur « modifié » */
    VMCU_STREAM_AUDIO_OUT   = 2, /* tampon circulaire firmware -> app (I2S) */
    VMCU_STREAM_AUDIO_IN    = 3, /* tampon circulaire app -> firmware (micro) */
    VMCU_STREAM_CAMERA      = 4, /* images app -> firmware */
    VMCU_STREAM_BULK        = 5  /* données SPI volumineuses (usage interne) */
} vmcu_stream_kind_t;

typedef enum {
    VMCU_PIXEL_NONE   = 0,
    VMCU_PIXEL_RGB565 = 1,
    VMCU_PIXEL_RGB888 = 2,
    VMCU_PIXEL_MONO1  = 3
} vmcu_pixel_format_t;

typedef struct {
    uint32_t            capacity;    /* octets de la zone de données ; 0 = calculé (framebuffer) */
    uint16_t            width;       /* framebuffer / caméra */
    uint16_t            height;
    vmcu_pixel_format_t format;
    uint32_t            sample_rate; /* audio, Hz */
    uint8_t             channels;    /* audio */
    uint8_t             bits;        /* audio : bits par échantillon */
} vmcu_stream_params_t;

typedef struct vmcu_stream vmcu_stream_t;

/**
 * Crée un segment de mémoire partagée et l'annonce à l'app (STREAM_OPEN,
 * synchrone). Capacité bornée à 16 Mio.
 */
vmcu_status_t vmcu_stream_open(vmcu_stream_kind_t kind, const vmcu_stream_params_t *params,
                               vmcu_stream_t **out);

/** Zone de données (framebuffer : écriture directe, puis vmcu_stream_commit). */
void *vmcu_stream_data(vmcu_stream_t *stream);

/** Capacité de la zone de données, en octets. */
size_t vmcu_stream_capacity(const vmcu_stream_t *stream);

/**
 * Producteur d'un tampon circulaire : copie au plus len octets, sans bloquer.
 * Renvoie le nombre d'octets écrits (0 si plein). Rend visible et notifie.
 */
size_t vmcu_stream_write(vmcu_stream_t *stream, const void *data, size_t len);

/** Consommateur d'un tampon circulaire : lit au plus len octets, sans bloquer. */
size_t vmcu_stream_read(vmcu_stream_t *stream, void *buf, size_t len);

/** Framebuffer : marque l'image « modifiée » et notifie l'app (notification coalescée). */
void vmcu_stream_commit(vmcu_stream_t *stream);

/** Ferme le flux (STREAM_CLOSE) et libère le segment. */
void vmcu_stream_close(vmcu_stream_t *stream);

/* ------------------------------------------------------------------------- */
/* Diagnostic                                                                */
/* ------------------------------------------------------------------------- */

typedef enum {
    VMCU_LOG_ERROR = 0,
    VMCU_LOG_WARN  = 1,
    VMCU_LOG_INFO  = 2,
    VMCU_LOG_DEBUG = 3
} vmcu_log_level_t;

/**
 * Signale une fonction du framework non simulée : « ⚠ HAL_xxx non simulé »
 * dans l'onglet ⚠. Une seule fois par nom de fonction (règle 16.2.10).
 */
void vmcu_warn_unsimulated(const char *function_name);

/** Journal vers l'app (texte UTF-8 tronqué à 1024 octets). */
void vmcu_log(vmcu_log_level_t level, const char *fmt, ...) VMCU_PRINTF_LIKE(2, 3);

typedef enum {
    VMCU_PING_CORE   = 0, /* firmware -> cœur Rust -> firmware */
    VMCU_PING_WORKER = 1  /* firmware -> cœur -> modèle du composant d'écho (QuickJS) -> firmware */
} vmcu_ping_target_t;

/**
 * Mesure un aller-retour synchrone (outil de latence, section 8.1).
 * VMCU_PING_CORE : le cœur répond lui-même.
 * VMCU_PING_WORKER : le cœur appelle onPing du modèle d'un composant d'écho
 * (components/echo/) placé sur la paillasse ; c'est le chemin réel d'une
 * transaction I2C/SPI synchrone. Le nom est historique (modèles d'abord
 * exécutés dans des Web Workers, aujourd'hui dans QuickJS au cœur). Sans composant d'écho :
 * VMCU_ERR_UNSUPPORTED.
 * @param rtt_us  reçoit la durée mesurée en µs (horloge réelle, pauses comprises).
 */
vmcu_status_t vmcu_ping(vmcu_ping_target_t target, uint32_t *rtt_us);

#ifdef __cplusplus
}
#endif

#endif /* VMCU_H */
