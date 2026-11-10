/*
 * libvmcu/core/vmcu_internal.h — Déclarations partagées entre les modules de libvmcu.
 * Rien ici ne fait partie de l'API publique (vmcu.h).
 */
#ifndef VMCU_INTERNAL_H
#define VMCU_INTERNAL_H

#include "vmcu.h"
#include "vmcu_platform.h"
#include "vmcu_wire.h"

/* Valeurs par défaut posées par CMake (VMCU_BOARD, VMCU_FRAMEWORK). */
#ifndef VMCU_DEFAULT_BOARD
#define VMCU_DEFAULT_BOARD "nucleo-f401re"
#endif
#ifndef VMCU_DEFAULT_FRAMEWORK
#define VMCU_DEFAULT_FRAMEWORK "stm32-hal"
#endif
#define VMCU_LIBVMCU_VERSION "0.1.0"

/* Garde-fous (section 5.5). */
#define VMCU_GUARD_CALLS       10000u   /* appels sans sommeil avant micro-pause */
#define VMCU_GUARD_POLLS        1000u   /* lectures identiques avant micro-pause */
#define VMCU_GUARD_PAUSE_NS  1000000ull /* durée de la micro-pause : 1 ms */

/* --- Client (vmcu_client.c) -------------------------------------------------- */

/** Initialise si nécessaire. VMCU_ERR_DISCONNECTED après vmcu_shutdown. */
vmcu_status_t vmcu__ensure_init(void);

/** Bloque le fil appelant tant que la simulation est en pause. */
void vmcu__checkpoint(void);

/** Ajoute une trame asynchrone au lot courant. */
vmcu_status_t vmcu__send(uint16_t type, const uint8_t *payload, size_t len);

/**
 * Transaction synchrone : envoie une trame SYNC et attend la trame REPLY de
 * même seq. reply_type et reply_len PEUVENT être NULL. timeout_ms == 0 : délai
 * par défaut annoncé par WELCOME.
 */
vmcu_status_t vmcu__request(uint16_t type, const uint8_t *payload, size_t len,
                            uint8_t *reply, size_t reply_cap, size_t *reply_len,
                            uint16_t *reply_type, uint32_t timeout_ms);

/** Vide le lot courant (avant un sommeil). */
void vmcu__flush(void);

/** Événement remis au fil IRQ. */
typedef struct {
    uint8_t port;
    uint8_t pin;
    uint8_t edge; /* vmcu_gpio_edge_t détecté */
} vmcu__irq_event_t;

/** Ajoute un événement à la file IRQ bornée. false si la file est pleine. */
bool vmcu__enqueue_irq(const vmcu__irq_event_t *ev);

/** Résolution ADC reçue dans WELCOME. */
uint8_t vmcu__adc_bits(void);

/* --- Temps et garde-fous (vmcu_time.c) ------------------------------------------- */

void     vmcu__time_init(void);
void     vmcu__time_set_paused(bool paused);
uint64_t vmcu__sim_ns(void);
void     vmcu__guard_call(void);
void     vmcu__guard_poll(bool changed);
void     vmcu__guard_reset(void);

/* --- GPIO (vmcu_gpio.c) ---------------------------------------------------------- */

void vmcu__gpio_init(void);
void vmcu__gpio_on_input(uint8_t port, uint8_t pin, uint8_t level);
void vmcu__gpio_dispatch(const vmcu__irq_event_t *ev);

/* --- Diagnostic (vmcu_diag.c) -------------------------------------------------- */

void vmcu__diag_init(void);
/** WARN vers l'app + ligne sur stderr. */
void vmcu__warn(uint16_t code, const char *text);
/** Comme vmcu__warn, mais une seule fois par `key` (nom de fonction). */
void vmcu__warn_once(uint16_t code, const char *key, const char *text);

#endif /* VMCU_INTERNAL_H */
