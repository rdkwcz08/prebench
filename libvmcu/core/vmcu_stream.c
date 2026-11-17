/*
 * libvmcu/core/vmcu_stream.c — Canal flux en mémoire partagée (vmcu.h ;
 * docs/protocol.md, section 6 ; phase 3, étape 6).
 *
 * Segment : en-tête de 64 octets, puis la zone de données. libvmcu crée le
 * segment, l'annonce par STREAM_OPEN (synchrone) et, une fois l'app abonnée,
 * en retire le nom : un firmware arrêté brutalement ne laisse aucun segment
 * derrière lui (POSIX ; sous Windows, il disparaît avec la dernière poignée).
 *
 * Positions et numéros de commit : atomiques C11, partagés entre processus
 * (accès sans verrou sur 64 bits, garantis sur les cibles de Prebench).
 * Un seul producteur et un seul consommateur par flux.
 */
#include "vmcu_internal.h"

#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>

#include "vmcu_platform.h"
#include "vmcu_wire.h"

#define HEADER_SIZE   64u
#define STREAM_MAGIC  0x4D534250u /* 'P' 'B' 'S' 'M' */
#define LAYOUT_V1     1u
#define MAX_CAPACITY  (16u * 1024u * 1024u)

/* Décalages des champs de l'en-tête (section 6.2). */
#define OFF_WRITE_POS  24u
#define OFF_READ_POS   32u
#define OFF_COMMIT_SEQ 40u
#define OFF_ACK_SEQ    44u

struct vmcu_stream {
    vmcu_plat_shm_t   *shm;
    uint8_t           *base;
    uint8_t           *data;
    uint32_t           capacity;
    uint16_t           id;
    vmcu_stream_kind_t kind;
};

static atomic_uint next_id = 1;

static _Atomic uint64_t *u64_at(const vmcu_stream_t *s, size_t off)
{
    return (_Atomic uint64_t *)(void *)(s->base + off);
}

static _Atomic uint32_t *u32_at(const vmcu_stream_t *s, size_t off)
{
    return (_Atomic uint32_t *)(void *)(s->base + off);
}

/** Capacité demandée, ou calculée pour un framebuffer ; 0 si invalide. */
static uint32_t capacity_for(vmcu_stream_kind_t kind, const vmcu_stream_params_t *p)
{
    uint64_t cap = p->capacity;
    if (cap == 0 && kind == VMCU_STREAM_FRAMEBUFFER) {
        uint64_t px = (uint64_t)p->width * p->height;
        switch (p->format) {
        case VMCU_PIXEL_RGB565: cap = px * 2u; break;
        case VMCU_PIXEL_RGB888: cap = px * 3u; break;
        case VMCU_PIXEL_MONO1:  cap = (px + 7u) / 8u; break;
        default:                cap = 0; break;
        }
    }
    return (cap == 0 || cap > MAX_CAPACITY) ? 0u : (uint32_t)cap;
}

static void notify(const vmcu_stream_t *s, uint32_t commit)
{
    uint8_t payload[8];
    vmcu_wbuf_t w;
    vmcu_wbuf_init(&w, payload, sizeof payload);
    vmcu_wbuf_u16(&w, s->id);
    vmcu_wbuf_u16(&w, 0);
    vmcu_wbuf_u32(&w, commit);
    (void)vmcu__send(VMCU_MSG_STREAM_NOTIFY, payload, w.len);
}

/** Nouveau commit ; notifie seulement si le consommateur était à jour (section 6.3). */
static void publish(vmcu_stream_t *s)
{
    uint32_t commit = atomic_fetch_add_explicit(u32_at(s, OFF_COMMIT_SEQ), 1u, memory_order_acq_rel) + 1u;
    uint32_t ack = atomic_load_explicit(u32_at(s, OFF_ACK_SEQ), memory_order_acquire);
    if (ack == commit - 1u) {
        notify(s, commit);
    }
}

vmcu_status_t vmcu_stream_open(vmcu_stream_kind_t kind, const vmcu_stream_params_t *params,
                               vmcu_stream_t **out)
{
    if (out == NULL) {
        return VMCU_ERR_INVALID_ARG;
    }
    *out = NULL;
    if (params == NULL || kind < VMCU_STREAM_FRAMEBUFFER || kind > VMCU_STREAM_BULK) {
        return VMCU_ERR_INVALID_ARG;
    }
    uint32_t capacity = capacity_for(kind, params);
    if (capacity == 0) {
        return VMCU_ERR_INVALID_ARG;
    }
    vmcu_status_t st = vmcu__ensure_init();
    if (st != VMCU_OK) {
        return st;
    }
    unsigned raw_id = atomic_fetch_add(&next_id, 1u);
    if (raw_id == 0 || raw_id > 0xFFFFu) {
        return VMCU_ERR_BUSY; /* 65 535 flux dans la session */
    }

    vmcu_stream_t *s = calloc(1, sizeof *s);
    if (s == NULL) {
        return VMCU_ERR_NO_MEMORY;
    }
    s->id = (uint16_t)raw_id;
    s->kind = kind;
    s->capacity = capacity;
    char name[64];
    vmcu_plat_shm_name(name, sizeof name, vmcu_plat_pid(), vmcu__board_id(), s->id);
    if (vmcu_plat_shm_create(name, (size_t)HEADER_SIZE + capacity, &s->shm) != VMCU_PLAT_OK) {
        free(s);
        return VMCU_ERR_NO_MEMORY;
    }
    s->base = vmcu_plat_shm_ptr(s->shm);
    s->data = s->base + HEADER_SIZE;

    /* En-tête (le segment neuf est à zéro : positions et numéros aussi). */
    vmcu_wire_put_u32(s->base + 0, STREAM_MAGIC);
    vmcu_wire_put_u16(s->base + 4, LAYOUT_V1);
    s->base[6] = (uint8_t)kind;
    s->base[7] = (uint8_t)params->format;
    vmcu_wire_put_u32(s->base + 8, capacity);
    vmcu_wire_put_u16(s->base + 12, params->width);
    vmcu_wire_put_u16(s->base + 14, params->height);
    vmcu_wire_put_u32(s->base + 16, params->sample_rate);
    s->base[20] = params->channels;
    s->base[21] = params->bits;
    atomic_thread_fence(memory_order_release);

    uint8_t payload[32 + 64];
    vmcu_wbuf_t w;
    vmcu_wbuf_init(&w, payload, sizeof payload);
    vmcu_wbuf_u16(&w, s->id);
    vmcu_wbuf_u8(&w, (uint8_t)kind);
    vmcu_wbuf_u8(&w, (uint8_t)params->format);
    vmcu_wbuf_u32(&w, capacity);
    vmcu_wbuf_u16(&w, params->width);
    vmcu_wbuf_u16(&w, params->height);
    vmcu_wbuf_u32(&w, params->sample_rate);
    vmcu_wbuf_u8(&w, params->channels);
    vmcu_wbuf_u8(&w, params->bits);
    vmcu_wbuf_u16(&w, 0);
    vmcu_wbuf_str(&w, name);

    uint8_t reply[8];
    size_t reply_len = 0;
    uint16_t reply_type = 0;
    st = vmcu__request(VMCU_MSG_STREAM_OPEN, payload, w.len, reply, sizeof reply, &reply_len,
                       &reply_type, 0);
    if (st == VMCU_OK && (reply_type != VMCU_MSG_STREAM_OPENED || reply_len < 4
                          || vmcu_wire_get_u16(reply) != s->id)) {
        st = VMCU_ERR_PROTOCOL;
    }
    if (st == VMCU_OK) {
        int16_t app_status = (int16_t)vmcu_wire_get_u16(reply + 2);
        st = (vmcu_status_t)app_status;
    }
    /* Ouvert par l'app (ou refusé) : le nom ne sert plus. */
    vmcu_plat_shm_unlink(s->shm);
    if (st != VMCU_OK) {
        vmcu_plat_shm_close(s->shm);
        free(s);
        return st;
    }
    *out = s;
    return VMCU_OK;
}

void *vmcu_stream_data(vmcu_stream_t *stream)
{
    return stream != NULL ? stream->data : NULL;
}

size_t vmcu_stream_capacity(const vmcu_stream_t *stream)
{
    return stream != NULL ? stream->capacity : 0u;
}

uint16_t vmcu__stream_id(const vmcu_stream_t *stream)
{
    return stream->id;
}

size_t vmcu__stream_free(const vmcu_stream_t *s)
{
    uint64_t wp = atomic_load_explicit(u64_at(s, OFF_WRITE_POS), memory_order_relaxed);
    uint64_t rp = atomic_load_explicit(u64_at(s, OFF_READ_POS), memory_order_acquire);
    uint64_t used = wp - rp;
    return used >= s->capacity ? 0u : (size_t)(s->capacity - used);
}

/** Copie len octets à la position courante du tampon circulaire (place déjà vérifiée). */
static uint64_t put(vmcu_stream_t *s, const uint8_t *data, size_t len)
{
    uint64_t wp = atomic_load_explicit(u64_at(s, OFF_WRITE_POS), memory_order_relaxed);
    size_t at = (size_t)(wp % s->capacity);
    size_t first = s->capacity - at;
    if (first > len) {
        first = len;
    }
    memcpy(s->data + at, data, first);
    memcpy(s->data, data + first, len - first);
    atomic_store_explicit(u64_at(s, OFF_WRITE_POS), wp + len, memory_order_release);
    return wp;
}

bool vmcu__stream_put(vmcu_stream_t *s, const uint8_t *data, size_t len, uint64_t *position)
{
    if (s == NULL || len > vmcu__stream_free(s)) {
        return false;
    }
    *position = put(s, data, len);
    return true;
}

size_t vmcu_stream_write(vmcu_stream_t *stream, const void *data, size_t len)
{
    if (stream == NULL || data == NULL || len == 0 || stream->kind == VMCU_STREAM_FRAMEBUFFER) {
        return 0;
    }
    size_t free_bytes = vmcu__stream_free(stream);
    size_t n = len < free_bytes ? len : free_bytes;
    if (n == 0) {
        return 0;
    }
    (void)put(stream, data, n);
    publish(stream);
    return n;
}

size_t vmcu_stream_read(vmcu_stream_t *stream, void *buf, size_t len)
{
    if (stream == NULL || buf == NULL || len == 0 || stream->kind == VMCU_STREAM_FRAMEBUFFER) {
        return 0;
    }
    uint64_t wp = atomic_load_explicit(u64_at(stream, OFF_WRITE_POS), memory_order_acquire);
    uint64_t rp = atomic_load_explicit(u64_at(stream, OFF_READ_POS), memory_order_relaxed);
    uint64_t avail = wp - rp;
    size_t n = avail < len ? (size_t)avail : len;
    size_t at = (size_t)(rp % stream->capacity);
    size_t first = stream->capacity - at;
    if (first > n) {
        first = n;
    }
    memcpy(buf, stream->data + at, first);
    memcpy((uint8_t *)buf + first, stream->data, n - first);
    atomic_store_explicit(u64_at(stream, OFF_READ_POS), rp + n, memory_order_release);
    /* Consommateur à jour : le producteur (l'app) notifiera le prochain commit. */
    uint32_t commit = atomic_load_explicit(u32_at(stream, OFF_COMMIT_SEQ), memory_order_acquire);
    atomic_store_explicit(u32_at(stream, OFF_ACK_SEQ), commit, memory_order_release);
    return n;
}

void vmcu_stream_commit(vmcu_stream_t *stream)
{
    if (stream != NULL && stream->kind == VMCU_STREAM_FRAMEBUFFER) {
        publish(stream);
    }
}

void vmcu_stream_close(vmcu_stream_t *stream)
{
    if (stream == NULL) {
        return;
    }
    uint8_t payload[2];
    vmcu_wire_put_u16(payload, stream->id);
    (void)vmcu__send(VMCU_MSG_STREAM_CLOSE, payload, sizeof payload);
    vmcu_plat_shm_close(stream->shm);
    free(stream);
}
