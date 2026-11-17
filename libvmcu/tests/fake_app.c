/*
 * libvmcu/tests/fake_app.c — Fausse app pour les tests (voir fake_app.h).
 */
#include "fake_app.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "vmcu_platform.h"
#include "vmcu_wire.h"

#define MAX_FRAMES  4096u
#define MAX_INITIAL 32u
#define MAX_STREAMS 4u
#define SHM_HEADER  64u

typedef struct {
    uint16_t         id; /* 0 : libre */
    uint32_t         capacity;
    vmcu_plat_shm_t *shm;
} fake_stream_t;

struct fake_app {
    uint16_t              board_id;
    char                  address[256];
    vmcu_plat_listener_t *listener;
    vmcu_plat_conn_t     *conn;
    vmcu_plat_thread_t   *thread;
    vmcu_plat_mutex_t    *mx;       /* frames, état */
    vmcu_plat_cond_t     *cv;
    vmcu_plat_mutex_t    *write_mx; /* écritures sur le canal */
    fake_frame_t          frames[MAX_FRAMES];
    unsigned              frame_count;
    uint8_t               initial[MAX_INITIAL][3];
    unsigned              initial_count;
    uint8_t               initial_adc[MAX_INITIAL][4]; /* unité, canal, valeur LE */
    unsigned              initial_adc_count;
    bool                  echo;
    uint8_t               max_minor;    /* version mineure du protocole de la fausse app */
    uint8_t               i2c_addr;     /* esclave I2C simulé ; 0 : aucun */
    uint8_t               i2c_mem[256]; /* registres de l'esclave */
    uint8_t               i2c_ptr;      /* pointeur de registre */
    /* SPI : esclave qui répond le complément de chaque octet reçu ; octets
     * émis (hors NO_TX) comptés et hachés dans l'ordre (FNV-1a). */
    uint64_t              spi_bytes;
    uint32_t              spi_hash;
    unsigned              spi_bulk;     /* transactions passées par le flux bulk */
    fake_stream_t         streams[MAX_STREAMS];
    uint32_t              seq;
    char                  hello_board[64];
    char                  hello_framework[64];
};

static void send_frame(fake_app_t *fa, uint16_t type, uint16_t flags, uint32_t seq,
                       const uint8_t *payload, size_t len)
{
    static uint8_t buf[VMCU_WIRE_HEADER_SIZE + 8 + 4096]; /* sous write_mx */
    vmcu_plat_mutex_lock(fa->write_mx);
    vmcu_frame_header_t h = {
        .type = type,
        .board_id = fa->board_id,
        .flags = flags,
        .seq = (flags & VMCU_FLAG_REPLY) ? seq : ++fa->seq,
        .length = (uint32_t)len,
        .timestamp_us = 0,
    };
    vmcu_wire_encode_header(buf, &h);
    if (len > 0) {
        memcpy(buf + VMCU_WIRE_HEADER_SIZE, payload, len);
    }
    (void)vmcu_plat_write_all(fa->conn, buf, VMCU_WIRE_HEADER_SIZE + len);
    vmcu_plat_mutex_unlock(fa->write_mx);
}

static bool read_exact(fake_app_t *fa, uint8_t *buf, size_t len)
{
    while (len > 0) {
        long n = vmcu_plat_read(fa->conn, buf, len);
        if (n <= 0) {
            return false;
        }
        buf += n;
        len -= (size_t)n;
    }
    return true;
}

static void handle_hello(fake_app_t *fa, const vmcu_frame_header_t *h, const uint8_t *p)
{
    vmcu_rbuf_t r;
    vmcu_rbuf_init(&r, p, h->length);
    uint8_t major = vmcu_rbuf_u8(&r);
    uint8_t minor = vmcu_rbuf_u8(&r);
    (void)vmcu_rbuf_u8(&r);
    (void)vmcu_rbuf_u8(&r);
    (void)vmcu_rbuf_u32(&r);
    vmcu_plat_mutex_lock(fa->mx);
    vmcu_rbuf_str(&r, fa->hello_board, sizeof fa->hello_board);
    vmcu_rbuf_str(&r, fa->hello_framework, sizeof fa->hello_framework);
    vmcu_plat_mutex_unlock(fa->mx);

    if (major != 1 || h->board_id != fa->board_id) {
        uint8_t reject[16];
        vmcu_wbuf_t w;
        vmcu_wbuf_init(&w, reject, sizeof reject);
        vmcu_wbuf_u16(&w, 1);
        vmcu_wbuf_str(&w, "test");
        send_frame(fa, VMCU_MSG_REJECT, VMCU_FLAG_REPLY, h->seq, reject, w.len);
        return;
    }
    for (unsigned i = 0; i < fa->initial_count; ++i) {
        uint8_t in[4] = { fa->initial[i][0], fa->initial[i][1], fa->initial[i][2], 0 };
        send_frame(fa, VMCU_MSG_GPIO_INPUT, 0, 0, in, sizeof in);
    }
    for (unsigned i = 0; i < fa->initial_adc_count; ++i) {
        send_frame(fa, VMCU_MSG_ADC_VALUE, 0, 0, fa->initial_adc[i], 4);
    }
    /* Mineure retenue : la plus petite des deux, comme le cœur. */
    uint8_t welcome[8] = { 1, minor < fa->max_minor ? minor : fa->max_minor, 12, 0 };
    vmcu_wire_put_u32(welcome + 4, 1000u);
    send_frame(fa, VMCU_MSG_WELCOME, VMCU_FLAG_REPLY, h->seq, welcome, sizeof welcome);
}

/*
 * I2C_XFER : esclave à registres, comme le joker I2C (écriture [reg, données…],
 * lecture à partir du pointeur, auto-incrément). Autre adresse : NACK.
 */
static void handle_i2c(fake_app_t *fa, const vmcu_frame_header_t *h, const uint8_t *p)
{
    uint8_t addr = p[1];
    uint16_t tx_len = vmcu_wire_get_u16(p + 2);
    uint16_t rx_len = vmcu_wire_get_u16(p + 4);
    const uint8_t *tx = p + 8;
    static uint8_t reply[4 + 4096];
    size_t n = 4;
    vmcu_plat_mutex_lock(fa->mx);
    bool ack = fa->i2c_addr != 0 && addr == fa->i2c_addr;
    if (ack) {
        if (tx_len > 0) {
            fa->i2c_ptr = tx[0];
            for (uint16_t i = 1; i < tx_len; ++i) {
                fa->i2c_mem[fa->i2c_ptr++] = tx[i];
            }
        }
        for (uint16_t i = 0; i < rx_len && i < 4096; ++i) {
            reply[n++] = fa->i2c_mem[fa->i2c_ptr++];
        }
    }
    vmcu_plat_mutex_unlock(fa->mx);
    reply[0] = ack ? 0 : 1;
    reply[1] = 0;
    vmcu_wire_put_u16(reply + 2, (uint16_t)(n - 4));
    send_frame(fa, VMCU_MSG_I2C_REPLY, VMCU_FLAG_REPLY, h->seq, reply, n);
}

static void spi_hash(fake_app_t *fa, const uint8_t *data, size_t len)
{
    for (size_t i = 0; i < len; ++i) {
        fa->spi_hash = (fa->spi_hash ^ data[i]) * 16777619u;
    }
    fa->spi_bytes += len;
}

static fake_stream_t *find_stream(fake_app_t *fa, uint16_t id)
{
    for (unsigned i = 0; i < MAX_STREAMS; ++i) {
        if (fa->streams[i].id == id && id != 0) {
            return &fa->streams[i];
        }
    }
    return NULL;
}

/* STREAM_OPEN : ouvre le segment, comme le cœur ; STREAM_OPENED avec le statut. */
static void handle_stream_open(fake_app_t *fa, const vmcu_frame_header_t *h, const uint8_t *p)
{
    vmcu_rbuf_t r;
    vmcu_rbuf_init(&r, p, h->length);
    uint16_t id = vmcu_rbuf_u16(&r);
    (void)vmcu_rbuf_u8(&r);
    (void)vmcu_rbuf_u8(&r);
    uint32_t capacity = vmcu_rbuf_u32(&r);
    for (int i = 0; i < 6; ++i) {
        (void)vmcu_rbuf_u16(&r); /* largeur, hauteur, fréquence (2 × u16), canaux et bits, réservé */
    }
    char name[64];
    vmcu_rbuf_str(&r, name, sizeof name);
    int16_t status = -7; /* VMCU_ERR_NO_MEMORY */
    vmcu_plat_mutex_lock(fa->mx);
    fake_stream_t *slot = find_stream(fa, 0);
    for (unsigned i = 0; i < MAX_STREAMS && slot == NULL; ++i) {
        if (fa->streams[i].id == 0) {
            slot = &fa->streams[i];
        }
    }
    if (slot != NULL && vmcu_plat_shm_open(name, SHM_HEADER + capacity, &slot->shm) == VMCU_PLAT_OK) {
        const uint8_t *base = vmcu_plat_shm_ptr(slot->shm);
        if (vmcu_wire_get_u32(base) == 0x4D534250u && vmcu_wire_get_u32(base + 8) == capacity) {
            slot->id = id;
            slot->capacity = capacity;
            status = 0;
        } else {
            vmcu_plat_shm_close(slot->shm);
            slot->shm = NULL;
            status = -9; /* VMCU_ERR_PROTOCOL */
        }
    }
    vmcu_plat_mutex_unlock(fa->mx);
    uint8_t reply[4];
    vmcu_wire_put_u16(reply, id);
    vmcu_wire_put_u16(reply + 2, (uint16_t)status);
    send_frame(fa, VMCU_MSG_STREAM_OPENED, VMCU_FLAG_REPLY, h->seq, reply, sizeof reply);
}

static void handle_stream_close(fake_app_t *fa, const uint8_t *p)
{
    vmcu_plat_mutex_lock(fa->mx);
    fake_stream_t *s = find_stream(fa, vmcu_wire_get_u16(p));
    if (s != NULL) {
        vmcu_plat_shm_close(s->shm);
        memset(s, 0, sizeof *s);
    }
    vmcu_plat_mutex_unlock(fa->mx);
}

/* SPI_XFER (section 4.3) : réponse = complément des octets émis (0x00 pour NO_TX). */
static void handle_spi(fake_app_t *fa, const vmcu_frame_header_t *h, const uint8_t *p)
{
    uint8_t flags = p[3];
    uint32_t len = vmcu_wire_get_u32(p + 4);
    static uint8_t reply[8 + 4096];
    vmcu_plat_mutex_lock(fa->mx);
    if (flags & 0x02u) {
        /* BULK : octets lus dans le flux, puis read_pos avancé (store release). */
        fake_stream_t *s = find_stream(fa, vmcu_wire_get_u16(p + 8));
        uint64_t pos = vmcu_wire_get_u64(p + 12);
        if (s != NULL) {
            uint8_t *base = vmcu_plat_shm_ptr(s->shm);
            for (uint32_t i = 0; i < len; ++i) {
                spi_hash(fa, base + SHM_HEADER + (size_t)((pos + i) % s->capacity), 1);
            }
            __atomic_store_n((uint64_t *)(void *)(base + 32), pos + len, __ATOMIC_RELEASE);
            fa->spi_bulk++;
        }
    } else if (!(flags & 0x04u)) {
        spi_hash(fa, p + 8, len);
    }
    vmcu_plat_mutex_unlock(fa->mx);
    if (flags & 0x01u) {
        reply[0] = 0;
        reply[1] = reply[2] = reply[3] = 0;
        vmcu_wire_put_u32(reply + 4, len);
        for (uint32_t i = 0; i < len && i < 4096; ++i) {
            reply[8 + i] = (flags & 0x04u) ? 0x00u : (uint8_t)~p[8 + i];
        }
        send_frame(fa, VMCU_MSG_SPI_REPLY, VMCU_FLAG_REPLY, h->seq, reply, 8 + (len < 4096 ? len : 4096));
    }
}

static void server_main(void *arg)
{
    fake_app_t *fa = arg;
    if (vmcu_plat_accept(fa->listener, &fa->conn) != VMCU_PLAT_OK) {
        fprintf(stderr, "fake_app : accept impossible\n");
        exit(1);
    }
    static uint8_t payload[VMCU_WIRE_MAX_PAYLOAD];
    uint8_t raw[VMCU_WIRE_HEADER_SIZE];
    while (read_exact(fa, raw, sizeof raw)) {
        vmcu_frame_header_t h;
        if (vmcu_wire_decode_header(raw, &h) != VMCU_WIRE_OK) {
            fprintf(stderr, "fake_app : en-tête invalide\n");
            exit(1);
        }
        if (h.length > 0 && !read_exact(fa, payload, h.length)) {
            break;
        }
        vmcu_plat_mutex_lock(fa->mx);
        if (fa->frame_count < MAX_FRAMES) {
            fake_frame_t *f = &fa->frames[fa->frame_count];
            memset(f, 0, sizeof *f);
            f->index = fa->frame_count;
            f->type = h.type;
            f->flags = h.flags;
            f->seq = h.seq;
            f->length = h.length;
            f->timestamp_us = h.timestamp_us;
            memcpy(f->data, payload, h.length < sizeof f->data ? h.length : sizeof f->data);
            fa->frame_count++;
        }
        bool echo = fa->echo;
        vmcu_plat_cond_broadcast(fa->cv);
        vmcu_plat_mutex_unlock(fa->mx);

        if (h.type == VMCU_MSG_HELLO) {
            handle_hello(fa, &h, payload);
        } else if (h.type == VMCU_MSG_I2C_XFER) {
            handle_i2c(fa, &h, payload);
        } else if (h.type == VMCU_MSG_SPI_XFER) {
            handle_spi(fa, &h, payload);
        } else if (h.type == VMCU_MSG_STREAM_OPEN) {
            handle_stream_open(fa, &h, payload);
        } else if (h.type == VMCU_MSG_STREAM_CLOSE) {
            handle_stream_close(fa, payload);
        } else if (h.type == VMCU_MSG_PING) {
            uint8_t target = (h.length > 0) ? payload[0] : 0;
            uint8_t pong[8] = { target, (target == 1 && !echo) ? 1 : 0, 0, 0, 0, 0, 0, 0 };
            send_frame(fa, VMCU_MSG_PONG, VMCU_FLAG_REPLY, h.seq, pong, sizeof pong);
        }
    }
}

fake_app_t *fake_app_create(uint16_t board_id)
{
    fake_app_t *fa = calloc(1, sizeof *fa);
    if (fa == NULL) {
        exit(1);
    }
    fa->board_id = board_id;
    fa->spi_hash = 2166136261u;
    fa->max_minor = 2;
    fa->mx = vmcu_plat_mutex_create(false);
    fa->cv = vmcu_plat_cond_create();
    fa->write_mx = vmcu_plat_mutex_create(false);
    return fa;
}

void fake_app_initial_adc(fake_app_t *fa, uint8_t unit, uint8_t channel, uint16_t raw)
{
    if (fa->initial_adc_count < MAX_INITIAL) {
        uint8_t *v = fa->initial_adc[fa->initial_adc_count++];
        v[0] = unit;
        v[1] = channel;
        v[2] = (uint8_t)(raw & 0xFF);
        v[3] = (uint8_t)(raw >> 8);
    }
}

void fake_app_initial_input(fake_app_t *fa, uint8_t port, uint8_t pin, uint8_t level)
{
    if (fa->initial_count < MAX_INITIAL) {
        fa->initial[fa->initial_count][0] = port;
        fa->initial[fa->initial_count][1] = pin;
        fa->initial[fa->initial_count][2] = level;
        fa->initial_count++;
    }
}

void fake_app_run(fake_app_t *fa)
{
    vmcu_plat_test_address(fa->address, sizeof fa->address);
    if (vmcu_plat_listen(fa->address, &fa->listener) != VMCU_PLAT_OK) {
        fprintf(stderr, "fake_app : écoute impossible sur %s\n", fa->address);
        exit(1);
    }
    char id[16];
    snprintf(id, sizeof id, "%u", (unsigned)fa->board_id);
    vmcu_plat_setenv("PREBENCH_SOCKET", fa->address);
    vmcu_plat_setenv("PREBENCH_BOARD_ID", id);
    fa->thread = vmcu_plat_thread_start(server_main, fa);
}

void fake_app_set_protocol_minor(fake_app_t *fa, uint8_t minor)
{
    fa->max_minor = minor;
}

void fake_app_set_echo(fake_app_t *fa, bool present)
{
    vmcu_plat_mutex_lock(fa->mx);
    fa->echo = present;
    vmcu_plat_mutex_unlock(fa->mx);
}

void fake_app_set_i2c_device(fake_app_t *fa, uint8_t addr7)
{
    vmcu_plat_mutex_lock(fa->mx);
    fa->i2c_addr = addr7;
    vmcu_plat_mutex_unlock(fa->mx);
}

void fake_app_set_i2c_reg(fake_app_t *fa, uint8_t reg, uint8_t value)
{
    vmcu_plat_mutex_lock(fa->mx);
    fa->i2c_mem[reg] = value;
    vmcu_plat_mutex_unlock(fa->mx);
}

void fake_app_spi_stats(fake_app_t *fa, uint64_t *bytes, uint32_t *hash, unsigned *bulk)
{
    vmcu_plat_mutex_lock(fa->mx);
    *bytes = fa->spi_bytes;
    *hash = fa->spi_hash;
    *bulk = fa->spi_bulk;
    vmcu_plat_mutex_unlock(fa->mx);
}

void fake_app_send_gpio_input(fake_app_t *fa, uint8_t port, uint8_t pin, uint8_t level)
{
    uint8_t in[4] = { port, pin, level, 0 };
    send_frame(fa, VMCU_MSG_GPIO_INPUT, 0, 0, in, sizeof in);
}

void fake_app_send_uart_rx(fake_app_t *fa, uint8_t id, const uint8_t *data, size_t len)
{
    while (len > 0) {
        size_t n = len > 60 ? 60 : len;
        uint8_t in[64] = { id, 0, (uint8_t)(n & 0xFF), (uint8_t)(n >> 8) };
        memcpy(in + 4, data, n);
        send_frame(fa, VMCU_MSG_UART_RX, 0, 0, in, 4 + n);
        data += n;
        len -= n;
    }
}

void fake_app_send_adc(fake_app_t *fa, uint8_t unit, uint8_t channel, uint16_t raw)
{
    uint8_t in[4] = { unit, channel, (uint8_t)(raw & 0xFF), (uint8_t)(raw >> 8) };
    send_frame(fa, VMCU_MSG_ADC_VALUE, 0, 0, in, sizeof in);
}

void fake_app_send_empty(fake_app_t *fa, uint16_t type)
{
    send_frame(fa, type, 0, 0, NULL, 0);
}

static bool matches(const fake_frame_t *f, uint16_t type, int port, int pin)
{
    return f->type == type && (port < 0 || f->data[0] == (uint8_t)port) &&
           (pin < 0 || f->data[1] == (uint8_t)pin);
}

static unsigned count_locked(fake_app_t *fa, uint16_t type, int port, int pin,
                             fake_frame_t *last)
{
    unsigned n = 0;
    for (unsigned i = 0; i < fa->frame_count; ++i) {
        if (matches(&fa->frames[i], type, port, pin)) {
            ++n;
            if (last != NULL) {
                *last = fa->frames[i];
            }
        }
    }
    return n;
}

bool fake_app_wait(fake_app_t *fa, uint16_t type, int port, int pin, unsigned count,
                   uint32_t timeout_ms, fake_frame_t *out)
{
    uint64_t deadline = vmcu_plat_monotonic_ns() + (uint64_t)timeout_ms * 1000000ull;
    vmcu_plat_mutex_lock(fa->mx);
    bool ok = false;
    for (;;) {
        if (count_locked(fa, type, port, pin, out) >= count) {
            ok = true;
            break;
        }
        uint64_t now = vmcu_plat_monotonic_ns();
        if (now >= deadline) {
            break;
        }
        (void)vmcu_plat_cond_timedwait(fa->cv, fa->mx, deadline - now);
    }
    vmcu_plat_mutex_unlock(fa->mx);
    return ok;
}

unsigned fake_app_count(fake_app_t *fa, uint16_t type, int port, int pin)
{
    vmcu_plat_mutex_lock(fa->mx);
    unsigned n = count_locked(fa, type, port, pin, NULL);
    vmcu_plat_mutex_unlock(fa->mx);
    return n;
}

const char *fake_app_hello_board(fake_app_t *fa)
{
    return fa->hello_board;
}

const char *fake_app_hello_framework(fake_app_t *fa)
{
    return fa->hello_framework;
}

void fake_app_stop(fake_app_t *fa)
{
    vmcu_plat_thread_join(fa->thread);
    vmcu_plat_conn_close(fa->conn);
    vmcu_plat_listener_close(fa->listener);
    free(fa);
}
