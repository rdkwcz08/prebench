/*
 * libvmcu/core/vmcu_wire.c — Encodage des trames du protocole v1.
 */
#include "vmcu_wire.h"

#include <string.h>

void vmcu_wire_put_u16(uint8_t *p, uint16_t v)
{
    p[0] = (uint8_t)(v & 0xFFu);
    p[1] = (uint8_t)(v >> 8);
}

void vmcu_wire_put_u32(uint8_t *p, uint32_t v)
{
    for (unsigned i = 0; i < 4u; ++i) {
        p[i] = (uint8_t)(v >> (8u * i));
    }
}

void vmcu_wire_put_u64(uint8_t *p, uint64_t v)
{
    for (unsigned i = 0; i < 8u; ++i) {
        p[i] = (uint8_t)(v >> (8u * i));
    }
}

uint16_t vmcu_wire_get_u16(const uint8_t *p)
{
    return (uint16_t)((uint16_t)p[0] | ((uint16_t)p[1] << 8));
}

uint32_t vmcu_wire_get_u32(const uint8_t *p)
{
    uint32_t v = 0;
    for (unsigned i = 0; i < 4u; ++i) {
        v |= (uint32_t)p[i] << (8u * i);
    }
    return v;
}

uint64_t vmcu_wire_get_u64(const uint8_t *p)
{
    uint64_t v = 0;
    for (unsigned i = 0; i < 8u; ++i) {
        v |= (uint64_t)p[i] << (8u * i);
    }
    return v;
}

void vmcu_wire_encode_header(uint8_t out[VMCU_WIRE_HEADER_SIZE], const vmcu_frame_header_t *h)
{
    vmcu_wire_put_u16(out + 0, VMCU_WIRE_MAGIC);
    vmcu_wire_put_u16(out + 2, h->type);
    vmcu_wire_put_u16(out + 4, h->board_id);
    vmcu_wire_put_u16(out + 6, h->flags);
    vmcu_wire_put_u32(out + 8, h->seq);
    vmcu_wire_put_u32(out + 12, h->length);
    vmcu_wire_put_u64(out + 16, h->timestamp_us);
}

vmcu_wire_result_t vmcu_wire_decode_header(const uint8_t in[VMCU_WIRE_HEADER_SIZE],
                                           vmcu_frame_header_t *h)
{
    if (vmcu_wire_get_u16(in + 0) != VMCU_WIRE_MAGIC) {
        return VMCU_WIRE_BAD_MAGIC;
    }
    h->type         = vmcu_wire_get_u16(in + 2);
    h->board_id     = vmcu_wire_get_u16(in + 4);
    h->flags        = vmcu_wire_get_u16(in + 6);
    h->seq          = vmcu_wire_get_u32(in + 8);
    h->length       = vmcu_wire_get_u32(in + 12);
    h->timestamp_us = vmcu_wire_get_u64(in + 16);
    if (h->length > VMCU_WIRE_MAX_PAYLOAD) {
        return VMCU_WIRE_TOO_LONG;
    }
    return VMCU_WIRE_OK;
}

/* --- Écriture ------------------------------------------------------------ */

void vmcu_wbuf_init(vmcu_wbuf_t *w, uint8_t *buf, size_t cap)
{
    w->buf = buf;
    w->cap = cap;
    w->len = 0;
    w->overflow = 0;
}

static uint8_t *wbuf_reserve(vmcu_wbuf_t *w, size_t n)
{
    if (w->overflow || n > w->cap - w->len) {
        w->overflow = 1;
        return NULL;
    }
    uint8_t *p = w->buf + w->len;
    w->len += n;
    return p;
}

void vmcu_wbuf_u8(vmcu_wbuf_t *w, uint8_t v)
{
    uint8_t *p = wbuf_reserve(w, 1);
    if (p != NULL) {
        p[0] = v;
    }
}

void vmcu_wbuf_u16(vmcu_wbuf_t *w, uint16_t v)
{
    uint8_t *p = wbuf_reserve(w, 2);
    if (p != NULL) {
        vmcu_wire_put_u16(p, v);
    }
}

void vmcu_wbuf_u32(vmcu_wbuf_t *w, uint32_t v)
{
    uint8_t *p = wbuf_reserve(w, 4);
    if (p != NULL) {
        vmcu_wire_put_u32(p, v);
    }
}

void vmcu_wbuf_u64(vmcu_wbuf_t *w, uint64_t v)
{
    uint8_t *p = wbuf_reserve(w, 8);
    if (p != NULL) {
        vmcu_wire_put_u64(p, v);
    }
}

void vmcu_wbuf_bytes(vmcu_wbuf_t *w, const void *data, size_t len)
{
    uint8_t *p = wbuf_reserve(w, len);
    if (p != NULL && len > 0) {
        memcpy(p, data, len);
    }
}

void vmcu_wbuf_str(vmcu_wbuf_t *w, const char *s)
{
    size_t n = (s != NULL) ? strlen(s) : 0;
    if (n > VMCU_WIRE_MAX_STR) {
        n = VMCU_WIRE_MAX_STR;
        /* Ne pas couper au milieu d'un caractère UTF-8 multi-octets. */
        while (n > 0 && ((uint8_t)s[n] & 0xC0u) == 0x80u) {
            --n;
        }
    }
    vmcu_wbuf_u16(w, (uint16_t)n);
    vmcu_wbuf_bytes(w, s, n);
}

/* --- Lecture ------------------------------------------------------------- */

void vmcu_rbuf_init(vmcu_rbuf_t *r, const uint8_t *p, size_t len)
{
    r->p = p;
    r->len = len;
    r->pos = 0;
    r->error = 0;
}

static const uint8_t *rbuf_take(vmcu_rbuf_t *r, size_t n)
{
    if (r->error || n > r->len - r->pos) {
        r->error = 1;
        return NULL;
    }
    const uint8_t *p = r->p + r->pos;
    r->pos += n;
    return p;
}

uint8_t vmcu_rbuf_u8(vmcu_rbuf_t *r)
{
    const uint8_t *p = rbuf_take(r, 1);
    return (p != NULL) ? p[0] : 0u;
}

uint16_t vmcu_rbuf_u16(vmcu_rbuf_t *r)
{
    const uint8_t *p = rbuf_take(r, 2);
    return (p != NULL) ? vmcu_wire_get_u16(p) : 0u;
}

uint32_t vmcu_rbuf_u32(vmcu_rbuf_t *r)
{
    const uint8_t *p = rbuf_take(r, 4);
    return (p != NULL) ? vmcu_wire_get_u32(p) : 0u;
}

uint64_t vmcu_rbuf_u64(vmcu_rbuf_t *r)
{
    const uint8_t *p = rbuf_take(r, 8);
    return (p != NULL) ? vmcu_wire_get_u64(p) : 0u;
}

void vmcu_rbuf_str(vmcu_rbuf_t *r, char *out, size_t cap)
{
    if (cap == 0) {
        return;
    }
    out[0] = '\0';
    uint16_t n = vmcu_rbuf_u16(r);
    const uint8_t *p = rbuf_take(r, n);
    if (p == NULL) {
        return;
    }
    size_t copy = (n < cap - 1) ? n : cap - 1;
    memcpy(out, p, copy);
    out[copy] = '\0';
}
