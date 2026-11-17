/*
 * libvmcu/core/vmcu_led_strip.c — LED adressables (vmcu.h, API 1.2 ;
 * docs/protocol.md, LED_STRIP, section 4.4 ; phase 3, étape 8).
 *
 * Une trame entière par appel, octets dans l'ordre du fil (GRB pour un
 * WS2812) : le timing du signal n'est pas simulé (section 5.4). Coalescence,
 * comme PWM_DUTY : tant qu'une trame de la même ligne attend dans le lot, la
 * nouvelle remplace sa charge utile (même longueur) ; une trame identique à la
 * précédente n'est pas renvoyée (empreinte FNV-1a sur 64 bits et longueur).
 * App en protocole 1.1 : rien n'est envoyé, VMCU_ERR_UNSUPPORTED et un
 * avertissement.
 */
#include "vmcu_internal.h"

#include <string.h>

#include "vmcu_platform.h"
#include "vmcu_wire.h"

/** Lignes suivies en même temps (borne, règle 16.2.11) ; au-delà, pas de coalescence. */
#define MAX_LINES 16

typedef struct {
    bool           used;
    uint8_t        key[4]; /* port, pin, unité, canal */
    uint64_t       hash;
    size_t         len;
    vmcu__latest_t pending;
} line_t;

static vmcu_plat_mutex_t *strip_mx;
static line_t lines[MAX_LINES];

void vmcu__led_strip_init(void)
{
    strip_mx = vmcu_plat_mutex_create(false);
}

static uint64_t fnv1a(const uint8_t *data, size_t len)
{
    uint64_t h = 1469598103934665603ull;
    for (size_t i = 0; i < len; ++i) {
        h = (h ^ data[i]) * 1099511628211ull;
    }
    return h;
}

static vmcu_status_t show(uint8_t port, uint8_t pin, uint8_t unit, uint8_t channel,
                          const uint8_t *data, size_t len)
{
    if ((data == NULL && len > 0) || len > VMCU_LED_STRIP_MAX_BYTES) {
        return VMCU_ERR_INVALID_ARG;
    }
    vmcu_status_t st = vmcu__ensure_init();
    if (st != VMCU_OK) {
        return st;
    }
    if (vmcu__proto_minor() < 2) {
        vmcu_warn_unsimulated("LED adressables (app trop ancienne : protocole 1.2 requis)");
        return VMCU_ERR_UNSUPPORTED;
    }
    uint8_t payload[8 + VMCU_LED_STRIP_MAX_BYTES];
    vmcu_wbuf_t w;
    vmcu_wbuf_init(&w, payload, sizeof payload);
    vmcu_wbuf_u8(&w, port);
    vmcu_wbuf_u8(&w, pin);
    vmcu_wbuf_u8(&w, unit);
    vmcu_wbuf_u8(&w, channel);
    vmcu_wbuf_u16(&w, (uint16_t)len);
    vmcu_wbuf_u16(&w, 0);
    if (len > 0) {
        vmcu_wbuf_bytes(&w, data, len);
    }

    const uint8_t key[4] = { port, pin, unit, channel };
    uint64_t hash = fnv1a(data, len);
    vmcu_plat_mutex_lock(strip_mx);
    line_t *line = NULL;
    for (int i = 0; i < MAX_LINES && line == NULL; ++i) {
        if (lines[i].used && memcmp(lines[i].key, key, sizeof key) == 0) {
            line = &lines[i];
        }
    }
    for (int i = 0; i < MAX_LINES && line == NULL; ++i) {
        if (!lines[i].used) {
            line = &lines[i];
            memset(line, 0, sizeof *line);
            line->used = true;
            memcpy(line->key, key, sizeof key);
            line->len = SIZE_MAX; /* aucune trame encore */
        }
    }
    if (line == NULL) {
        /* Plus de 16 lignes : envoi simple, sans coalescence. */
        vmcu_plat_mutex_unlock(strip_mx);
        return vmcu__send(VMCU_MSG_LED_STRIP, payload, w.len);
    }
    if (line->len == len && line->hash == hash) {
        vmcu_plat_mutex_unlock(strip_mx);
        return VMCU_OK; /* trame identique à la précédente */
    }
    line->len = len;
    line->hash = hash;
    st = vmcu__send_latest(VMCU_MSG_LED_STRIP, payload, w.len, &line->pending);
    vmcu_plat_mutex_unlock(strip_mx);
    return st;
}

vmcu_status_t vmcu_led_strip_show(uint8_t port, uint8_t pin, const uint8_t *data, size_t len)
{
    if (port >= VMCU_GPIO_MAX_PORTS || pin >= VMCU_GPIO_MAX_PINS) {
        return VMCU_ERR_INVALID_ARG;
    }
    return show(port, pin, VMCU_PIN_NONE, VMCU_PIN_NONE, data, len);
}

vmcu_status_t vmcu_led_strip_show_pwm(uint8_t unit, uint8_t channel, const uint8_t *data,
                                      size_t len)
{
    if (channel < 1 || channel > 4) {
        return VMCU_ERR_INVALID_ARG;
    }
    return show(VMCU_PIN_NONE, VMCU_PIN_NONE, unit, channel, data, len);
}
