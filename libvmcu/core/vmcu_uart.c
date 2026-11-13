/*
 * libvmcu/core/vmcu_uart.c — UART (vmcu.h ; docs/protocol.md, UART_CONFIG, UART_TX, UART_RX).
 *
 * - Émission : trames UART_TX de 4 Kio au plus, envoyées par lots (non bloquant).
 * - Réception : le fil E/S range les octets de UART_RX dans un tampon borné de
 *   VMCU_UART_RX_BUFFER_SIZE par UART. Tampon plein : octets perdus et WARN
 *   code 2, comme un dépassement (overrun) sur une vraie carte.
 * - Callback (vmcu_uart_on_rx) : les octets reçus lui sont remis depuis le fil
 *   IRQ, verrou d'interruption tenu ; ils ne restent pas dans le tampon.
 */
#include "vmcu_internal.h"

#include <string.h>

#define UART_IDS 16u
#define UART_TX_CHUNK 4096u

typedef struct {
    uint8_t         buf[VMCU_UART_RX_BUFFER_SIZE];
    size_t          head;
    size_t          count;
    vmcu_uart_rx_cb cb;
    void           *user;
    bool            posted;    /* événement IRQ déjà en file */
    bool            overflowed; /* dépassement signalé, jusqu'à ce que le tampon se vide */
} uart_t;

static uart_t uarts[UART_IDS];
static vmcu_plat_mutex_t *uart_mx;
static vmcu_plat_cond_t *uart_cv;
static vmcu_plat_mutex_t *tx_mx; /* une trame UART_TX à la fois (tampon partagé) */

void vmcu__uart_init(void)
{
    uart_mx = vmcu_plat_mutex_create(false);
    uart_cv = vmcu_plat_cond_create();
    tx_mx = vmcu_plat_mutex_create(false);
}

vmcu_status_t vmcu_uart_config(uint8_t id, uint32_t baud)
{
    if (id >= UART_IDS) {
        return VMCU_ERR_INVALID_ARG;
    }
    vmcu_status_t st = vmcu__ensure_init();
    if (st != VMCU_OK) {
        return st;
    }
    vmcu__guard_call();
    uint8_t payload[8];
    vmcu_wbuf_t w;
    vmcu_wbuf_init(&w, payload, sizeof payload);
    vmcu_wbuf_u8(&w, id);
    vmcu_wbuf_u8(&w, 0);
    vmcu_wbuf_u8(&w, 0);
    vmcu_wbuf_u8(&w, 0);
    vmcu_wbuf_u32(&w, baud);
    return vmcu__send(VMCU_MSG_UART_CONFIG, payload, w.len);
}

vmcu_status_t vmcu_uart_write(uint8_t id, const uint8_t *data, size_t len)
{
    if (id >= UART_IDS || (data == NULL && len > 0)) {
        return VMCU_ERR_INVALID_ARG;
    }
    vmcu_status_t st = vmcu__ensure_init();
    if (st != VMCU_OK) {
        return st;
    }
    vmcu__guard_call();
    static uint8_t payload[4 + UART_TX_CHUNK];
    vmcu_plat_mutex_lock(tx_mx);
    while (len > 0 && st == VMCU_OK) {
        size_t n = (len > UART_TX_CHUNK) ? UART_TX_CHUNK : len;
        vmcu_wbuf_t w;
        vmcu_wbuf_init(&w, payload, sizeof payload);
        vmcu_wbuf_u8(&w, id);
        vmcu_wbuf_u8(&w, 0);
        vmcu_wbuf_u16(&w, (uint16_t)n);
        vmcu_wbuf_bytes(&w, data, n);
        st = vmcu__send(VMCU_MSG_UART_TX, payload, w.len);
        data += n;
        len -= n;
    }
    vmcu_plat_mutex_unlock(tx_mx);
    return st;
}

/* Fil E/S : octets reçus de l'app (UART_RX). */
void vmcu__uart_on_rx(uint8_t id, const uint8_t *data, size_t len)
{
    if (id >= UART_IDS || uart_mx == NULL) {
        return;
    }
    uart_t *u = &uarts[id];
    bool lost = false;
    bool post = false;
    vmcu_plat_mutex_lock(uart_mx);
    for (size_t i = 0; i < len; ++i) {
        if (u->count == VMCU_UART_RX_BUFFER_SIZE) {
            lost = true;
            continue;
        }
        u->buf[(u->head + u->count) % VMCU_UART_RX_BUFFER_SIZE] = data[i];
        u->count++;
    }
    if (lost && !u->overflowed) {
        u->overflowed = true;
    } else {
        lost = false; /* déjà signalé pour ce dépassement */
    }
    if (u->cb != NULL && !u->posted && u->count > 0) {
        u->posted = true;
        post = true;
    }
    vmcu_plat_cond_broadcast(uart_cv);
    vmcu_plat_mutex_unlock(uart_mx);
    if (lost) {
        vmcu__warn(VMCU_WARN_UART_OVERRUN, "UART : tampon de réception plein, octets perdus (dépassement)");
    }
    if (post) {
        vmcu__irq_event_t ev = { .kind = VMCU__IRQ_UART, .port = id };
        if (!vmcu__enqueue_irq(&ev)) {
            vmcu_plat_mutex_lock(uart_mx);
            u->posted = false;
            vmcu_plat_mutex_unlock(uart_mx);
        }
    }
}

/* Fil IRQ : remet au callback les octets en attente. */
void vmcu__uart_dispatch(uint8_t id)
{
    if (id >= UART_IDS) {
        return;
    }
    static uint8_t tmp[VMCU_UART_RX_BUFFER_SIZE]; /* fil IRQ unique */
    uart_t *u = &uarts[id];
    vmcu_plat_mutex_lock(uart_mx);
    size_t n = u->count;
    for (size_t i = 0; i < n; ++i) {
        tmp[i] = u->buf[(u->head + i) % VMCU_UART_RX_BUFFER_SIZE];
    }
    u->head = (u->head + n) % VMCU_UART_RX_BUFFER_SIZE;
    u->count = 0;
    u->overflowed = false;
    u->posted = false;
    vmcu_uart_rx_cb cb = u->cb;
    void *user = u->user;
    vmcu_plat_mutex_unlock(uart_mx);
    if (cb != NULL && n > 0) {
        cb(id, tmp, n, user);
    }
}

size_t vmcu_uart_read(uint8_t id, uint8_t *buf, size_t max, uint32_t timeout_ms)
{
    if (id >= UART_IDS || buf == NULL || max == 0 || vmcu__ensure_init() != VMCU_OK) {
        return 0;
    }
    vmcu__checkpoint();
    uart_t *u = &uarts[id];
    uint64_t deadline = vmcu_plat_monotonic_ns() + (uint64_t)timeout_ms * 1000000ull;
    vmcu_plat_mutex_lock(uart_mx);
    while (u->count == 0 && timeout_ms > 0) {
        uint64_t now = vmcu_plat_monotonic_ns();
        if (now >= deadline) {
            break;
        }
        (void)vmcu_plat_cond_timedwait(uart_cv, uart_mx, deadline - now);
    }
    size_t n = (u->count < max) ? u->count : max;
    for (size_t i = 0; i < n; ++i) {
        buf[i] = u->buf[(u->head + i) % VMCU_UART_RX_BUFFER_SIZE];
    }
    u->head = (u->head + n) % VMCU_UART_RX_BUFFER_SIZE;
    u->count -= n;
    if (u->count == 0) {
        u->overflowed = false;
    }
    vmcu_plat_mutex_unlock(uart_mx);
    vmcu__guard_poll(n > 0); /* lecture sans attente en boucle : micro-pauses */
    return n;
}

size_t vmcu_uart_available(uint8_t id)
{
    if (id >= UART_IDS || vmcu__ensure_init() != VMCU_OK) {
        return 0;
    }
    vmcu_plat_mutex_lock(uart_mx);
    size_t n = uarts[id].count;
    vmcu_plat_mutex_unlock(uart_mx);
    vmcu__guard_poll(n > 0);
    return n;
}

vmcu_status_t vmcu_uart_on_rx(uint8_t id, vmcu_uart_rx_cb cb, void *user)
{
    if (id >= UART_IDS) {
        return VMCU_ERR_INVALID_ARG;
    }
    vmcu_status_t st = vmcu__ensure_init();
    if (st != VMCU_OK) {
        return st;
    }
    bool post = false;
    vmcu_plat_mutex_lock(uart_mx);
    uarts[id].cb = cb;
    uarts[id].user = user;
    if (cb != NULL && uarts[id].count > 0 && !uarts[id].posted) {
        uarts[id].posted = true; /* octets déjà reçus : remis au callback */
        post = true;
    }
    vmcu_plat_mutex_unlock(uart_mx);
    if (post) {
        vmcu__irq_event_t ev = { .kind = VMCU__IRQ_UART, .port = id };
        (void)vmcu__enqueue_irq(&ev);
    }
    return VMCU_OK;
}
