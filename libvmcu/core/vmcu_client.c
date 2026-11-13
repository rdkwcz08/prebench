/*
 * libvmcu/core/vmcu_client.c — Connexion à l'app, lots, transactions synchrones,
 * fils E/S, IRQ et vidage (docs/protocol.md sections 3 et 5).
 *
 * Fils internes :
 * - E/S     : lit le canal, complète les transactions, alimente caches et file IRQ ;
 * - IRQ     : exécute les callbacks, un à la fois, verrou d'interruption tenu ;
 * - vidage  : envoie le lot au plus 1 ms après sa première trame ; il dort
 *             sans délai quand le lot est vide (aucune activité au repos) ;
 * - débogage : seulement si PREBENCH_DEBUG=1 (firmware lancé par le débogueur,
 *             protocole 1.1) : lit l'horloge toutes les 100 ms et envoie
 *             HEARTBEAT toutes les 200 ms ; le temps où tout le processus
 *             était figé (point d'arrêt) est retiré de l'horloge par le
 *             premier fil qui la lit ensuite (vmcu_time.c) : aucun saut.
 *
 * Ordre des verrous : send_mx -> sync_mx. Le fil E/S ne prend jamais send_mx
 * en tenant sync_mx.
 */
#include "vmcu_internal.h"

#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define SEND_BUF_SIZE     (VMCU_WIRE_MAX_PAYLOAD + 2u * VMCU_WIRE_HEADER_SIZE)
#define SYNC_SLOTS        32
#define IRQ_QUEUE_SIZE    256
#define HELLO_TIMEOUT_MS  5000u
#define CONNECT_TIMEOUT_NS 5000000000ull
#define FLUSH_DELAY_NS    1000000ull
#define DEFAULT_SYNC_TIMEOUT_MS 1000u
#define DEBUG_TICK_NS     100000000ull /* réveil du fil de débogage */
#define DEBUG_FROZEN_NS   250000000ull /* écart entre deux lectures : processus figé */

enum { STATE_NONE = 0, STATE_STARTING = 1, STATE_READY = 2, STATE_CLOSED = 3 };

typedef struct {
    bool      armed;     /* seq attribué, réponse attendue */
    bool      in_use;
    bool      done;
    bool      failed;    /* canal fermé avant la réponse */
    uint32_t  seq;
    uint16_t  reply_type;
    uint8_t  *buf;
    size_t    cap;
    size_t    len;
} sync_slot_t;

static struct {
    atomic_int        state;
    atomic_bool       closing;
    vmcu_board_id_t   board_id;
    uint8_t           adc_bits;
    uint32_t          sync_timeout_ms;
    vmcu_plat_conn_t *conn;

    /* Émission */
    vmcu_plat_mutex_t *send_mx;
    vmcu_plat_cond_t  *send_cv;
    uint8_t            send_buf[SEND_BUF_SIZE];
    size_t             send_len;
    uint64_t           first_pending_ns;
    uint32_t           seq;
    bool               flusher_stop;

    /* Transactions synchrones */
    vmcu_plat_mutex_t *sync_mx;
    vmcu_plat_cond_t  *sync_cv;
    sync_slot_t        slots[SYNC_SLOTS];

    /* File IRQ */
    vmcu_plat_mutex_t *irq_q_mx;
    vmcu_plat_cond_t  *irq_q_cv;
    vmcu__irq_event_t  irq_q[IRQ_QUEUE_SIZE];
    size_t             irq_head;
    size_t             irq_count;
    bool               irq_stop;
    vmcu_plat_mutex_t *irq_lock; /* verrou d'interruption, récursif */

    /* Pause */
    vmcu_plat_mutex_t *pause_mx;
    vmcu_plat_cond_t  *pause_cv;
    atomic_bool        paused;

    vmcu_plat_thread_t *io_thread;
    vmcu_plat_thread_t *irq_thread;
    vmcu_plat_thread_t *flush_thread;
    vmcu_plat_thread_t *debug_thread;
    uint8_t            proto_minor; /* mineure retenue à la poignée de main */

    uint8_t unknown_seen[8192]; /* types inconnus déjà journalisés (1 bit par type) */
} g;

static uint8_t g_rx_payload[VMCU_WIRE_MAX_PAYLOAD];

/* --- Sorties fatales ----------------------------------------------------------- */

static VMCU_NORETURN void die(int code, const char *message)
{
    fprintf(stderr, "prebench : %s\n", message);
    fflush(stderr);
    _Exit(code);
}

/* --- Émission ------------------------------------------------------------------- */

static void flush_locked(void)
{
    if (g.send_len == 0) {
        return;
    }
    size_t len = g.send_len;
    g.send_len = 0;
    if (vmcu_plat_write_all(g.conn, g.send_buf, len) != 0 && !atomic_load(&g.closing)) {
        die(VMCU_EXIT_DISCONNECTED, "connexion à l'app perdue, arrêt du firmware");
    }
}

/* Ajoute une trame au lot ; si slot != NULL, la trame est SYNC et le slot est armé. */
static vmcu_status_t send_frame(uint16_t type, uint16_t flags, const uint8_t *payload,
                                size_t len, sync_slot_t *slot)
{
    if (len > VMCU_WIRE_MAX_PAYLOAD) {
        return VMCU_ERR_INVALID_ARG;
    }
    if (atomic_load(&g.closing)) {
        return VMCU_ERR_DISCONNECTED;
    }
    vmcu_plat_mutex_lock(g.send_mx);
    if (g.send_len + VMCU_WIRE_HEADER_SIZE + len > SEND_BUF_SIZE) {
        flush_locked();
    }
    bool was_empty = (g.send_len == 0);
    vmcu_frame_header_t h = {
        .type = type,
        .board_id = g.board_id,
        .flags = flags,
        .seq = ++g.seq,
        .length = (uint32_t)len,
        .timestamp_us = vmcu__sim_ns() / 1000u,
    };
    vmcu_wire_encode_header(g.send_buf + g.send_len, &h);
    g.send_len += VMCU_WIRE_HEADER_SIZE;
    if (len > 0) {
        memcpy(g.send_buf + g.send_len, payload, len);
        g.send_len += len;
    }
    if (slot != NULL) {
        vmcu_plat_mutex_lock(g.sync_mx);
        slot->seq = h.seq;
        slot->armed = true;
        vmcu_plat_mutex_unlock(g.sync_mx);
        flush_locked(); /* une requête SYNC part avec tout le lot qui la précède */
    } else if (was_empty) {
        g.first_pending_ns = vmcu_plat_monotonic_ns();
        vmcu_plat_cond_signal(g.send_cv);
    }
    vmcu_plat_mutex_unlock(g.send_mx);
    return VMCU_OK;
}

vmcu_status_t vmcu__send(uint16_t type, const uint8_t *payload, size_t len)
{
    return send_frame(type, 0, payload, len, NULL);
}

void vmcu__flush(void)
{
    if (g.send_mx == NULL) {
        return;
    }
    vmcu_plat_mutex_lock(g.send_mx);
    flush_locked();
    vmcu_plat_mutex_unlock(g.send_mx);
}

static void flusher_main(void *arg)
{
    (void)arg;
    vmcu_plat_mutex_lock(g.send_mx);
    while (!g.flusher_stop) {
        if (g.send_len == 0) {
            vmcu_plat_cond_wait(g.send_cv, g.send_mx);
            continue;
        }
        uint64_t elapsed = vmcu_plat_monotonic_ns() - g.first_pending_ns;
        if (elapsed >= FLUSH_DELAY_NS) {
            flush_locked();
        } else {
            (void)vmcu_plat_cond_timedwait(g.send_cv, g.send_mx, FLUSH_DELAY_NS - elapsed);
        }
    }
    vmcu_plat_mutex_unlock(g.send_mx);
}

/* --- Transactions synchrones ------------------------------------------------------- */

vmcu_status_t vmcu__request(uint16_t type, const uint8_t *payload, size_t len,
                            uint8_t *reply, size_t reply_cap, size_t *reply_len,
                            uint16_t *reply_type, uint32_t timeout_ms)
{
    if (atomic_load(&g.closing)) {
        return VMCU_ERR_DISCONNECTED;
    }
    /* Réserve un emplacement (attend si tous sont pris). */
    vmcu_plat_mutex_lock(g.sync_mx);
    sync_slot_t *slot = NULL;
    while (slot == NULL) {
        for (int i = 0; i < SYNC_SLOTS; ++i) {
            if (!g.slots[i].in_use) {
                slot = &g.slots[i];
                break;
            }
        }
        if (slot == NULL) {
            vmcu_plat_cond_wait(g.sync_cv, g.sync_mx);
        }
    }
    memset(slot, 0, sizeof *slot);
    slot->in_use = true;
    slot->buf = reply;
    slot->cap = reply_cap;
    vmcu_plat_mutex_unlock(g.sync_mx);

    vmcu_status_t st = send_frame(type, VMCU_FLAG_SYNC, payload, len, slot);
    if (st != VMCU_OK) {
        vmcu_plat_mutex_lock(g.sync_mx);
        slot->in_use = false;
        vmcu_plat_cond_broadcast(g.sync_cv);
        vmcu_plat_mutex_unlock(g.sync_mx);
        return st;
    }

    /* Le délai se compte en temps de simulation : il ne s'écoule pas en pause. */
    uint32_t t_ms = (timeout_ms != 0) ? timeout_ms : g.sync_timeout_ms;
    uint64_t deadline = vmcu__sim_ns() + (uint64_t)t_ms * 1000000ull;

    vmcu_plat_mutex_lock(g.sync_mx);
    while (!slot->done) {
        uint64_t now = vmcu__sim_ns();
        if (now >= deadline) {
            break;
        }
        uint64_t wait = deadline - now;
        if (wait > 50000000ull) {
            wait = 50000000ull; /* réévalue régulièrement (pause) */
        }
        (void)vmcu_plat_cond_timedwait(g.sync_cv, g.sync_mx, wait);
    }
    if (!slot->done) {
        st = VMCU_ERR_TIMEOUT;
    } else if (slot->failed) {
        st = VMCU_ERR_DISCONNECTED;
    } else {
        if (reply_len != NULL) {
            *reply_len = slot->len;
        }
        if (reply_type != NULL) {
            *reply_type = slot->reply_type;
        }
    }
    slot->armed = false;
    slot->in_use = false;
    vmcu_plat_cond_broadcast(g.sync_cv);
    vmcu_plat_mutex_unlock(g.sync_mx);
    return st;
}

static void complete_reply(const vmcu_frame_header_t *h, const uint8_t *payload)
{
    vmcu_plat_mutex_lock(g.sync_mx);
    for (int i = 0; i < SYNC_SLOTS; ++i) {
        sync_slot_t *s = &g.slots[i];
        if (s->in_use && s->armed && !s->done && s->seq == h->seq) {
            size_t n = (h->length < s->cap) ? h->length : s->cap;
            if (n > 0) {
                memcpy(s->buf, payload, n);
            }
            s->len = n;
            s->reply_type = h->type;
            s->done = true;
            vmcu_plat_cond_broadcast(g.sync_cv);
            break;
        }
    }
    /* Réponse tardive (après expiration) : ignorée. */
    vmcu_plat_mutex_unlock(g.sync_mx);
}

static void fail_all_requests(void)
{
    vmcu_plat_mutex_lock(g.sync_mx);
    for (int i = 0; i < SYNC_SLOTS; ++i) {
        if (g.slots[i].in_use && !g.slots[i].done) {
            g.slots[i].done = true;
            g.slots[i].failed = true;
        }
    }
    vmcu_plat_cond_broadcast(g.sync_cv);
    vmcu_plat_mutex_unlock(g.sync_mx);
}

/* --- File IRQ ------------------------------------------------------------------------ */

bool vmcu__enqueue_irq(const vmcu__irq_event_t *ev)
{
    vmcu_plat_mutex_lock(g.irq_q_mx);
    bool ok = g.irq_count < IRQ_QUEUE_SIZE;
    if (ok) {
        g.irq_q[(g.irq_head + g.irq_count) % IRQ_QUEUE_SIZE] = *ev;
        g.irq_count++;
        vmcu_plat_cond_signal(g.irq_q_cv);
    }
    vmcu_plat_mutex_unlock(g.irq_q_mx);
    return ok;
}

static void irq_main(void *arg)
{
    (void)arg;
    for (;;) {
        vmcu_plat_mutex_lock(g.irq_q_mx);
        while (g.irq_count == 0 && !g.irq_stop) {
            vmcu_plat_cond_wait(g.irq_q_cv, g.irq_q_mx);
        }
        if (g.irq_stop) {
            vmcu_plat_mutex_unlock(g.irq_q_mx);
            return;
        }
        vmcu__irq_event_t ev = g.irq_q[g.irq_head];
        g.irq_head = (g.irq_head + 1) % IRQ_QUEUE_SIZE;
        g.irq_count--;
        vmcu_plat_mutex_unlock(g.irq_q_mx);

        vmcu__checkpoint(); /* aucune interruption pendant une pause */
        vmcu_plat_mutex_lock(g.irq_lock);
        vmcu__gpio_dispatch(&ev);
        vmcu_plat_mutex_unlock(g.irq_lock);
    }
}

void vmcu_irq_lock(void)
{
    (void)vmcu__ensure_init();
    if (g.irq_lock != NULL) {
        vmcu_plat_mutex_lock(g.irq_lock);
    }
}

void vmcu_irq_unlock(void)
{
    if (g.irq_lock != NULL) {
        vmcu_plat_mutex_unlock(g.irq_lock);
    }
}

/* --- Pause ----------------------------------------------------------------------------- */

static void set_paused(bool on)
{
    vmcu__time_set_paused(on);
    vmcu_plat_mutex_lock(g.pause_mx);
    atomic_store(&g.paused, on);
    if (!on) {
        vmcu_plat_cond_broadcast(g.pause_cv);
    }
    vmcu_plat_mutex_unlock(g.pause_mx);
}

void vmcu__checkpoint(void)
{
    if (!atomic_load(&g.paused)) {
        return;
    }
    vmcu_plat_mutex_lock(g.pause_mx);
    while (atomic_load(&g.paused)) {
        vmcu_plat_cond_wait(g.pause_cv, g.pause_mx);
    }
    vmcu_plat_mutex_unlock(g.pause_mx);
}

/* --- Fil E/S ---------------------------------------------------------------------------- */

static bool read_exact(uint8_t *buf, size_t len)
{
    while (len > 0) {
        long n = vmcu_plat_read(g.conn, buf, len);
        if (n <= 0) {
            return false;
        }
        buf += n;
        len -= (size_t)n;
    }
    return true;
}

static void log_unknown_type(uint16_t type)
{
    uint8_t bit = (uint8_t)(1u << (type & 7u));
    if ((g.unknown_seen[type >> 3] & bit) == 0) {
        g.unknown_seen[type >> 3] |= bit;
        fprintf(stderr, "prebench : type de message inconnu 0x%04X ignoré\n", (unsigned)type);
    }
}

static VMCU_NORETURN void exit_after_flush(int code)
{
    vmcu_plat_mutex_lock(g.send_mx);
    flush_locked();
    vmcu_plat_mutex_unlock(g.send_mx);
    fflush(stdout);
    fflush(stderr);
    _Exit(code);
}

static void dispatch(const vmcu_frame_header_t *h, const uint8_t *payload)
{
    if ((h->flags & VMCU_FLAG_REPLY) != 0) {
        complete_reply(h, payload);
        return;
    }
    vmcu_rbuf_t r;
    vmcu_rbuf_init(&r, payload, h->length);
    switch (h->type) {
    case VMCU_MSG_GPIO_INPUT: {
        uint8_t port = vmcu_rbuf_u8(&r);
        uint8_t pin = vmcu_rbuf_u8(&r);
        uint8_t level = vmcu_rbuf_u8(&r);
        if (!r.error) {
            vmcu__gpio_on_input(port, pin, level);
        }
        break;
    }
    case VMCU_MSG_PAUSE:
        set_paused(true);
        break;
    case VMCU_MSG_RESUME:
        set_paused(false);
        break;
    case VMCU_MSG_RESET:
        exit_after_flush(VMCU_EXIT_RESET);
    case VMCU_MSG_SHUTDOWN:
        exit_after_flush(VMCU_EXIT_SHUTDOWN);
    default:
        log_unknown_type(h->type);
        break;
    }
}

static void io_main(void *arg)
{
    (void)arg;
    uint8_t raw[VMCU_WIRE_HEADER_SIZE];
    for (;;) {
        if (!read_exact(raw, sizeof raw)) {
            break;
        }
        vmcu_frame_header_t h;
        if (vmcu_wire_decode_header(raw, &h) != VMCU_WIRE_OK || h.board_id != g.board_id) {
            die(VMCU_EXIT_PROTOCOL, "trame invalide reçue de l'app (erreur de protocole)");
        }
        if (h.length > 0 && !read_exact(g_rx_payload, h.length)) {
            break;
        }
        dispatch(&h, g_rx_payload);
    }
    /* Canal fermé. */
    if (atomic_load(&g.closing)) {
        fail_all_requests();
        return;
    }
    die(VMCU_EXIT_DISCONNECTED, "connexion à l'app fermée, arrêt du firmware");
}

/* --- Cycle de vie ---------------------------------------------------------------------------- */

static void create_primitives(void)
{
    g.send_mx = vmcu_plat_mutex_create(false);
    g.send_cv = vmcu_plat_cond_create();
    g.sync_mx = vmcu_plat_mutex_create(false);
    g.sync_cv = vmcu_plat_cond_create();
    g.irq_q_mx = vmcu_plat_mutex_create(false);
    g.irq_q_cv = vmcu_plat_cond_create();
    g.irq_lock = vmcu_plat_mutex_create(true);
    g.pause_mx = vmcu_plat_mutex_create(false);
    g.pause_cv = vmcu_plat_cond_create();
    if (g.send_mx == NULL || g.send_cv == NULL || g.sync_mx == NULL || g.sync_cv == NULL ||
        g.irq_q_mx == NULL || g.irq_q_cv == NULL || g.irq_lock == NULL ||
        g.pause_mx == NULL || g.pause_cv == NULL) {
        die(VMCU_EXIT_PROTOCOL, "mémoire insuffisante à l'initialisation de libvmcu");
    }
}

static vmcu_board_id_t read_board_id(void)
{
    const char *s = getenv("PREBENCH_BOARD_ID");
    char *end = NULL;
    unsigned long v = (s != NULL) ? strtoul(s, &end, 10) : 0;
    if (s == NULL || *s == '\0' || end == NULL || *end != '\0' || v == 0 || v > 65535u) {
        die(VMCU_EXIT_NO_APP, "PREBENCH_BOARD_ID absent ou invalide : lancer le firmware depuis Prebench");
    }
    return (vmcu_board_id_t)v;
}

static void connect_to_app(const char *address)
{
    uint64_t deadline = vmcu_plat_monotonic_ns() + CONNECT_TIMEOUT_NS;
    for (;;) {
        vmcu_plat_result_t r = vmcu_plat_connect(address, &g.conn);
        if (r == VMCU_PLAT_OK) {
            return;
        }
        if (r == VMCU_PLAT_ERROR || vmcu_plat_monotonic_ns() >= deadline) {
            die(VMCU_EXIT_NO_APP, "impossible de joindre l'app (PREBENCH_SOCKET)");
        }
        vmcu_plat_sleep_ns(20000000ull);
    }
}

static void handshake(const char *board, const char *framework)
{
    uint8_t buf[3 * (VMCU_WIRE_MAX_STR + 2) + 8];
    vmcu_wbuf_t w;
    vmcu_wbuf_init(&w, buf, sizeof buf);
    vmcu_wbuf_u8(&w, VMCU_PROTOCOL_VERSION_MAJOR);
    vmcu_wbuf_u8(&w, VMCU_PROTOCOL_VERSION_MINOR);
    vmcu_wbuf_u8(&w, VMCU_API_VERSION_MAJOR);
    vmcu_wbuf_u8(&w, VMCU_API_VERSION_MINOR);
    vmcu_wbuf_u32(&w, vmcu_plat_pid());
    vmcu_wbuf_str(&w, board);
    vmcu_wbuf_str(&w, framework);
    vmcu_wbuf_str(&w, VMCU_LIBVMCU_VERSION);

    uint8_t reply[VMCU_WIRE_MAX_STR + 16];
    size_t reply_len = 0;
    uint16_t reply_type = 0;
    vmcu_status_t st = vmcu__request(VMCU_MSG_HELLO, buf, w.len, reply, sizeof reply,
                                     &reply_len, &reply_type, HELLO_TIMEOUT_MS);
    if (st != VMCU_OK) {
        die(VMCU_EXIT_PROTOCOL, "pas de réponse de l'app à la poignée de main");
    }
    vmcu_rbuf_t r;
    vmcu_rbuf_init(&r, reply, reply_len);
    if (reply_type == VMCU_MSG_REJECT) {
        uint16_t reason = vmcu_rbuf_u16(&r);
        char text[VMCU_WIRE_MAX_STR + 1];
        vmcu_rbuf_str(&r, text, sizeof text);
        fprintf(stderr, "prebench : connexion refusée par l'app (raison %u) : %s\n",
                (unsigned)reason, text);
        fflush(stderr);
        _Exit(VMCU_EXIT_PROTOCOL);
    }
    if (reply_type != VMCU_MSG_WELCOME) {
        die(VMCU_EXIT_PROTOCOL, "réponse inattendue à la poignée de main");
    }
    uint8_t major = vmcu_rbuf_u8(&r);
    g.proto_minor = vmcu_rbuf_u8(&r); /* mineure retenue */
    g.adc_bits = vmcu_rbuf_u8(&r);
    (void)vmcu_rbuf_u8(&r);
    uint32_t timeout = vmcu_rbuf_u32(&r);
    if (r.error || major != VMCU_PROTOCOL_VERSION_MAJOR) {
        die(VMCU_EXIT_PROTOCOL, "version de protocole incompatible avec l'app");
    }
    g.sync_timeout_ms = (timeout != 0) ? timeout : DEFAULT_SYNC_TIMEOUT_MS;
}

/* Fil de débogage : un débogueur fige tous les fils ; au réveil suivant, le
 * retard anormal est retiré de l'horloge, et l'absence de HEARTBEAT pendant
 * l'arrêt permet à l'app d'afficher « Arrêté sur un point d'arrêt ». */
static void debug_main(void *arg)
{
    (void)arg;
    unsigned ticks = 0;
    vmcu__time_watch_start(DEBUG_TICK_NS, DEBUG_FROZEN_NS);
    while (!atomic_load(&g.closing) && atomic_load(&g.state) == STATE_READY) {
        vmcu_plat_sleep_ns(DEBUG_TICK_NS);
        bool frozen = vmcu__time_watch_tick();
        if ((++ticks & 1u) == 0 || frozen) {
            (void)vmcu__send(VMCU_MSG_HEARTBEAT, NULL, 0);
        }
    }
}

static bool debug_requested(void)
{
    const char *v = getenv("PREBENCH_DEBUG");
    return v != NULL && strcmp(v, "1") == 0;
}

vmcu_status_t vmcu_init(const char *board, const char *framework)
{
    int expected = STATE_NONE;
    if (!atomic_compare_exchange_strong(&g.state, &expected, STATE_STARTING)) {
        while (atomic_load(&g.state) == STATE_STARTING) {
            vmcu_plat_sleep_ns(1000000ull);
        }
        return (atomic_load(&g.state) == STATE_READY) ? VMCU_OK : VMCU_ERR_DISCONNECTED;
    }
    if (board == NULL || *board == '\0') {
        board = VMCU_DEFAULT_BOARD;
    }
    if (framework == NULL || *framework == '\0') {
        framework = VMCU_DEFAULT_FRAMEWORK;
    }
    const char *address = getenv("PREBENCH_SOCKET");
    if (address == NULL || *address == '\0') {
        die(VMCU_EXIT_NO_APP, "PREBENCH_SOCKET absent : ce firmware se lance depuis Prebench");
    }
    g.board_id = read_board_id();
    g.sync_timeout_ms = DEFAULT_SYNC_TIMEOUT_MS;

    vmcu__time_init();
    vmcu__gpio_init();
    vmcu__diag_init();
    create_primitives();
    connect_to_app(address);

    g.io_thread = vmcu_plat_thread_start(io_main, NULL);
    g.irq_thread = vmcu_plat_thread_start(irq_main, NULL);
    g.flush_thread = vmcu_plat_thread_start(flusher_main, NULL);
    if (g.io_thread == NULL || g.irq_thread == NULL || g.flush_thread == NULL) {
        die(VMCU_EXIT_PROTOCOL, "impossible de créer les fils de libvmcu");
    }

    handshake(board, framework);
    atomic_store(&g.state, STATE_READY);
    if (debug_requested() && g.proto_minor >= 1) {
        g.debug_thread = vmcu_plat_thread_start(debug_main, NULL);
    }

    char priority[128];
    vmcu_plat_priority(priority, sizeof priority);
    vmcu_log(VMCU_LOG_INFO, "priorité du firmware : %s", priority);
    return VMCU_OK;
}

vmcu_status_t vmcu__ensure_init(void)
{
    int s = atomic_load(&g.state);
    if (s == STATE_READY) {
        return VMCU_OK;
    }
    if (s == STATE_CLOSED) {
        return VMCU_ERR_DISCONNECTED;
    }
    return vmcu_init(NULL, NULL);
}

void vmcu_shutdown(void)
{
    int expected = STATE_READY;
    if (!atomic_compare_exchange_strong(&g.state, &expected, STATE_CLOSED)) {
        return;
    }
    vmcu_plat_mutex_lock(g.send_mx);
    flush_locked();
    atomic_store(&g.closing, true);
    g.flusher_stop = true;
    vmcu_plat_cond_broadcast(g.send_cv);
    vmcu_plat_mutex_unlock(g.send_mx);
    vmcu_plat_thread_join(g.flush_thread);
    g.flush_thread = NULL;
    if (g.debug_thread != NULL) {
        vmcu_plat_thread_join(g.debug_thread); /* se termine au réveil suivant */
        g.debug_thread = NULL;
    }

    vmcu_plat_mutex_lock(g.irq_q_mx);
    g.irq_stop = true;
    vmcu_plat_cond_broadcast(g.irq_q_cv);
    vmcu_plat_mutex_unlock(g.irq_q_mx);
    if (!vmcu_plat_thread_is_current(g.irq_thread)) {
        vmcu_plat_thread_join(g.irq_thread);
        g.irq_thread = NULL;
    }

    set_paused(false); /* libère d'éventuels fils bloqués en pause */
    vmcu_plat_conn_shutdown(g.conn);
    vmcu_plat_thread_join(g.io_thread);
    g.io_thread = NULL;
    fail_all_requests();
    vmcu_plat_conn_close(g.conn);
    g.conn = NULL;
}

vmcu_board_id_t vmcu_board_id(void)
{
    return (atomic_load(&g.state) == STATE_READY) ? g.board_id : 0;
}

uint8_t vmcu__adc_bits(void)
{
    return g.adc_bits;
}

uint8_t vmcu_adc_bits(void)
{
    (void)vmcu__ensure_init();
    return g.adc_bits;
}

/* --- Ping (section 8.1) ------------------------------------------------------------------- */

vmcu_status_t vmcu_ping(vmcu_ping_target_t target, uint32_t *rtt_us)
{
    vmcu_status_t st = vmcu__ensure_init();
    if (st != VMCU_OK) {
        return st;
    }
    if (target != VMCU_PING_CORE && target != VMCU_PING_WORKER) {
        return VMCU_ERR_INVALID_ARG;
    }
    vmcu__checkpoint();
    uint8_t payload[4] = { (uint8_t)target, 0, 0, 0 };
    uint8_t reply[8];
    size_t reply_len = 0;
    uint16_t reply_type = 0;
    uint64_t t0 = vmcu_plat_monotonic_ns();
    st = vmcu__request(VMCU_MSG_PING, payload, sizeof payload, reply, sizeof reply,
                       &reply_len, &reply_type, 0);
    uint64_t t1 = vmcu_plat_monotonic_ns();
    if (st != VMCU_OK) {
        return st;
    }
    if (reply_type != VMCU_MSG_PONG || reply_len < 2) {
        return VMCU_ERR_PROTOCOL;
    }
    if (reply[1] != 0) {
        return VMCU_ERR_UNSUPPORTED; /* aucun composant d'écho */
    }
    if (rtt_us != NULL) {
        uint64_t us = (t1 - t0) / 1000u;
        *rtt_us = (us > UINT32_MAX) ? UINT32_MAX : (uint32_t)us;
    }
    return VMCU_OK;
}

/* --- Messages d'erreur ------------------------------------------------------------------- */

const char *vmcu_strerror(vmcu_status_t status)
{
    switch (status) {
    case VMCU_OK:                  return "succès";
    case VMCU_ERR_NOT_INITIALIZED: return "libvmcu n'est pas initialisée";
    case VMCU_ERR_INVALID_ARG:     return "argument invalide";
    case VMCU_ERR_UNSUPPORTED:     return "fonction non disponible ou non simulée";
    case VMCU_ERR_NACK:            return "non acquitté (NACK)";
    case VMCU_ERR_TIMEOUT:         return "délai dépassé";
    case VMCU_ERR_DISCONNECTED:    return "connexion à l'app fermée";
    case VMCU_ERR_NO_MEMORY:       return "mémoire insuffisante";
    case VMCU_ERR_BUFFER_FULL:     return "tampon plein";
    case VMCU_ERR_PROTOCOL:        return "erreur de protocole";
    case VMCU_ERR_VERSION:         return "version de protocole refusée";
    case VMCU_ERR_BUSY:            return "ressource déjà utilisée";
    }
    return "erreur inconnue";
}
