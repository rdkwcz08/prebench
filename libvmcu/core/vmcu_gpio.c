/*
 * libvmcu/core/vmcu_gpio.c — GPIO : configuration, écriture, cache des entrées, fronts.
 *
 * Les lectures sont servies par un cache local mis à jour par GPIO_INPUT :
 * aucun aller-retour avec l'app (docs/protocol.md, section 4.2).
 */
#include "vmcu_internal.h"

#include <stdatomic.h>

typedef struct {
    atomic_uchar      level;
    uint8_t           mode;
    uint8_t           pull;
    uint8_t           edge;
    uint8_t           af;
    vmcu_gpio_edge_cb cb;
    void             *user;
} pin_state_t;

static pin_state_t pins[VMCU_GPIO_MAX_PORTS][VMCU_GPIO_MAX_PINS];
static vmcu_plat_mutex_t *gpio_mx; /* protège mode, pull, edge, af, cb, user */

/* Dernière broche lue par ce fil, pour détecter l'attente active. */
static _Thread_local int t_last_key = -1;
static _Thread_local uint8_t t_last_level;

void vmcu__gpio_init(void)
{
    gpio_mx = vmcu_plat_mutex_create(false);
    for (unsigned p = 0; p < VMCU_GPIO_MAX_PORTS; ++p) {
        for (unsigned i = 0; i < VMCU_GPIO_MAX_PINS; ++i) {
            pins[p][i].af = VMCU_GPIO_AF_NONE;
        }
    }
}

static bool valid(uint8_t port, uint8_t pin)
{
    return port < VMCU_GPIO_MAX_PORTS && pin < VMCU_GPIO_MAX_PINS;
}

static vmcu_status_t send_config(uint8_t port, uint8_t pin, const pin_state_t *s)
{
    uint8_t payload[6] = { port, pin, s->mode, s->pull, s->edge, s->af };
    return vmcu__send(VMCU_MSG_GPIO_CONFIG, payload, sizeof payload);
}

vmcu_status_t vmcu_gpio_config(uint8_t port, uint8_t pin, vmcu_gpio_mode_t mode,
                               vmcu_gpio_pull_t pull, uint8_t af)
{
    vmcu_status_t st = vmcu__ensure_init();
    if (st != VMCU_OK) {
        return st;
    }
    if (!valid(port, pin) || (unsigned)mode > VMCU_GPIO_MODE_DISABLED ||
        (unsigned)pull > VMCU_GPIO_PULL_DOWN) {
        return VMCU_ERR_INVALID_ARG;
    }
    vmcu__checkpoint();
    vmcu_plat_mutex_lock(gpio_mx);
    pin_state_t *s = &pins[port][pin];
    s->mode = (uint8_t)mode;
    s->pull = (uint8_t)pull;
    s->af = af;
    pin_state_t copy = *s;
    vmcu_plat_mutex_unlock(gpio_mx);
    return send_config(port, pin, &copy);
}

vmcu_status_t vmcu_gpio_write(uint8_t port, uint8_t pin, vmcu_level_t level)
{
    vmcu_status_t st = vmcu__ensure_init();
    if (st != VMCU_OK) {
        return st;
    }
    if (!valid(port, pin)) {
        return VMCU_ERR_INVALID_ARG;
    }
    vmcu__checkpoint();
    vmcu__guard_call();
    uint8_t v = (level != VMCU_LOW) ? 1u : 0u;
    atomic_store(&pins[port][pin].level, v);
    uint8_t payload[3] = { port, pin, v };
    return vmcu__send(VMCU_MSG_GPIO_WRITE, payload, sizeof payload);
}

vmcu_level_t vmcu_gpio_read(uint8_t port, uint8_t pin)
{
    if (vmcu__ensure_init() != VMCU_OK || !valid(port, pin)) {
        return VMCU_LOW;
    }
    vmcu__checkpoint();
    vmcu__guard_call();
    uint8_t v = atomic_load(&pins[port][pin].level);
    int key = (int)port * (int)VMCU_GPIO_MAX_PINS + (int)pin;
    vmcu__guard_poll(key != t_last_key || v != t_last_level);
    t_last_key = key;
    t_last_level = v;
    return (v != 0) ? VMCU_HIGH : VMCU_LOW;
}

vmcu_status_t vmcu_gpio_on_edge(uint8_t port, uint8_t pin, vmcu_gpio_edge_t edge,
                                vmcu_gpio_edge_cb cb, void *user)
{
    vmcu_status_t st = vmcu__ensure_init();
    if (st != VMCU_OK) {
        return st;
    }
    if (!valid(port, pin) || (unsigned)edge > VMCU_GPIO_EDGE_BOTH) {
        return VMCU_ERR_INVALID_ARG;
    }
    vmcu__checkpoint();
    vmcu_plat_mutex_lock(gpio_mx);
    pin_state_t *s = &pins[port][pin];
    if (cb == NULL) {
        edge = VMCU_GPIO_EDGE_NONE;
    }
    s->edge = (uint8_t)edge;
    s->cb = (edge != VMCU_GPIO_EDGE_NONE) ? cb : NULL;
    s->user = user;
    pin_state_t copy = *s;
    vmcu_plat_mutex_unlock(gpio_mx);
    return send_config(port, pin, &copy);
}

/* Fil E/S : nouvel état d'une entrée. */
void vmcu__gpio_on_input(uint8_t port, uint8_t pin, uint8_t level)
{
    if (!valid(port, pin)) {
        return;
    }
    uint8_t v = (level != 0) ? 1u : 0u;
    uint8_t old = atomic_exchange(&pins[port][pin].level, v);
    if (old == v) {
        return;
    }
    vmcu_plat_mutex_lock(gpio_mx);
    uint8_t edge = pins[port][pin].edge;
    bool has_cb = pins[port][pin].cb != NULL;
    vmcu_plat_mutex_unlock(gpio_mx);

    uint8_t detected = v ? (uint8_t)VMCU_GPIO_EDGE_RISING : (uint8_t)VMCU_GPIO_EDGE_FALLING;
    if (has_cb && (edge & detected) != 0) {
        vmcu__irq_event_t ev = { .kind = VMCU__IRQ_GPIO, .port = port, .pin = pin, .edge = detected };
        if (!vmcu__enqueue_irq(&ev)) {
            vmcu__warn_once(VMCU_WARN_GENERIC, "irq-overflow",
                            "file d'interruptions pleine : fronts ignorés");
        }
    }
}

/* Fil IRQ, verrou d'interruption tenu. */
void vmcu__gpio_dispatch(const vmcu__irq_event_t *ev)
{
    vmcu_plat_mutex_lock(gpio_mx);
    vmcu_gpio_edge_cb cb = pins[ev->port][ev->pin].cb;
    void *user = pins[ev->port][ev->pin].user;
    uint8_t edge = pins[ev->port][ev->pin].edge;
    vmcu_plat_mutex_unlock(gpio_mx);
    if (cb != NULL && (edge & ev->edge) != 0) {
        cb(ev->port, ev->pin, (vmcu_gpio_edge_t)ev->edge, user);
    }
}
