/*
 * libvmcu/core/vmcu_spi.c — SPI maître (vmcu.h ; docs/protocol.md, SPI_CONFIG,
 * SPI_XFER, SPI_REPLY, section 4.3 ; phase 3, étape 6).
 *
 * Une écriture entière = un message (section 6.1 du cahier des charges) :
 * - avec réception : trame SYNC, le firmware attend la réponse du modèle du
 *   composant sélectionné ; au-delà de 4096 octets, plusieurs trames SYNC
 *   consécutives (la ligne CS ne change pas entre elles) ;
 * - écriture seule de 4096 octets au plus : trame asynchrone, groupée dans le
 *   lot courant, sans attente ;
 * - écriture seule plus grande (trame d'écran TFT) : données copiées dans le
 *   flux « bulk » en mémoire partagée (règle 16.2.5), trame asynchrone qui en
 *   donne la position. S'il manque de place, un PING au cœur attend qu'il ait
 *   traité les trames précédentes (donc lu leurs octets) : aucune scrutation.
 */
#include "vmcu_internal.h"

#include <string.h>

#include "vmcu_platform.h"
#include "vmcu_wire.h"

#define XFER_RX    0x01u
#define XFER_BULK  0x02u
#define XFER_NO_TX 0x04u

/** Capacité initiale du flux bulk (section 6.1) : plus qu'une trame TFT 320 × 240 en RGB565. */
#define BULK_DEFAULT (256u * 1024u)
#define BULK_MAX     (16u * 1024u * 1024u)

static vmcu_plat_mutex_t *bulk_mx;
static vmcu_stream_t *bulk;

void vmcu__spi_init(void)
{
    bulk_mx = vmcu_plat_mutex_create(false);
}

vmcu_status_t vmcu_spi_config(uint8_t bus, vmcu_spi_mode_t mode, uint32_t freq_hz, bool lsb_first)
{
    if (mode > VMCU_SPI_MODE_3) {
        return VMCU_ERR_INVALID_ARG;
    }
    vmcu_status_t st = vmcu__ensure_init();
    if (st != VMCU_OK) {
        return st;
    }
    uint8_t payload[8];
    vmcu_wbuf_t w;
    vmcu_wbuf_init(&w, payload, sizeof payload);
    vmcu_wbuf_u8(&w, bus);
    vmcu_wbuf_u8(&w, (uint8_t)mode);
    vmcu_wbuf_u8(&w, lsb_first ? 1u : 0u);
    vmcu_wbuf_u8(&w, 0);
    vmcu_wbuf_u32(&w, freq_hz);
    return vmcu__send(VMCU_MSG_SPI_CONFIG, payload, w.len);
}

static void header(vmcu_wbuf_t *w, uint8_t bus, uint8_t cs_port, uint8_t cs_pin, uint8_t flags,
                   size_t len)
{
    vmcu_wbuf_u8(w, bus);
    vmcu_wbuf_u8(w, cs_port);
    vmcu_wbuf_u8(w, cs_pin);
    vmcu_wbuf_u8(w, flags);
    vmcu_wbuf_u32(w, (uint32_t)len);
}

/** Transaction synchrone de VMCU_SPI_INLINE_MAX octets au plus. */
static vmcu_status_t xfer_sync(uint8_t bus, uint8_t cs_port, uint8_t cs_pin, const uint8_t *tx,
                               uint8_t *rx, size_t len, uint32_t timeout_ms)
{
    uint8_t payload[8 + VMCU_SPI_INLINE_MAX];
    vmcu_wbuf_t w;
    vmcu_wbuf_init(&w, payload, sizeof payload);
    header(&w, bus, cs_port, cs_pin, (uint8_t)(XFER_RX | (tx == NULL ? XFER_NO_TX : 0u)), len);
    if (tx != NULL) {
        vmcu_wbuf_bytes(&w, tx, len);
    }
    uint8_t reply[8 + VMCU_SPI_INLINE_MAX];
    size_t reply_len = 0;
    uint16_t reply_type = 0;
    vmcu_status_t st = vmcu__request(VMCU_MSG_SPI_XFER, payload, w.len, reply, sizeof reply,
                                     &reply_len, &reply_type, timeout_ms);
    if (st != VMCU_OK) {
        return st;
    }
    if (reply_type != VMCU_MSG_SPI_REPLY || reply_len < 8) {
        return VMCU_ERR_PROTOCOL;
    }
    if (reply[0] != 0) {
        return VMCU_ERR_PROTOCOL; /* erreur de bus signalée par l'app */
    }
    size_t got = vmcu_wire_get_u32(reply + 4);
    size_t avail = reply_len - 8;
    size_t n = got < avail ? got : avail;
    if (n > len) {
        n = len;
    }
    memcpy(rx, reply + 8, n);
    if (n < len) {
        memset(rx + n, 0xFF, len - n); /* le cœur complète déjà ; défense en profondeur */
    }
    return VMCU_OK;
}

/** Écriture seule de VMCU_SPI_INLINE_MAX octets au plus (tx NULL : 0xFF émis). */
static vmcu_status_t write_inline(uint8_t bus, uint8_t cs_port, uint8_t cs_pin, const uint8_t *tx,
                                  size_t len)
{
    uint8_t payload[8 + VMCU_SPI_INLINE_MAX];
    vmcu_wbuf_t w;
    vmcu_wbuf_init(&w, payload, sizeof payload);
    header(&w, bus, cs_port, cs_pin, tx == NULL ? XFER_NO_TX : 0u, len);
    if (tx != NULL) {
        vmcu_wbuf_bytes(&w, tx, len);
    }
    return vmcu__send(VMCU_MSG_SPI_XFER, payload, w.len);
}

/** Plus petite puissance de deux >= len, au moins BULK_DEFAULT. */
static uint32_t bulk_capacity(size_t len)
{
    uint32_t cap = BULK_DEFAULT;
    while (cap < len) {
        cap *= 2u;
    }
    return cap;
}

/** Écriture seule de plus de VMCU_SPI_INLINE_MAX octets, par le flux bulk. */
static vmcu_status_t write_bulk(uint8_t bus, uint8_t cs_port, uint8_t cs_pin, const uint8_t *tx,
                                size_t len)
{
    if (len > BULK_MAX) {
        return VMCU_ERR_INVALID_ARG;
    }
    vmcu_plat_mutex_lock(bulk_mx);
    vmcu_status_t st = VMCU_OK;
    if (bulk != NULL && vmcu_stream_capacity(bulk) < len) {
        /* Trop petit : remplacé. Le cœur garde l'ancien segment jusqu'à
         * STREAM_CLOSE, traité après les transactions qui le précèdent. */
        vmcu_stream_close(bulk);
        bulk = NULL;
    }
    if (bulk == NULL) {
        vmcu_stream_params_t params = { .capacity = bulk_capacity(len) };
        st = vmcu_stream_open(VMCU_STREAM_BULK, &params, &bulk);
    }
    uint64_t position = 0;
    if (st == VMCU_OK && !vmcu__stream_put(bulk, tx, len, &position)) {
        /* Plein : le cœur lit les octets en traitant chaque transaction ; une
         * fois le PING rendu, il a traité toutes les trames précédentes. */
        st = vmcu_ping(VMCU_PING_CORE, NULL);
        if (st == VMCU_OK && !vmcu__stream_put(bulk, tx, len, &position)) {
            st = VMCU_ERR_BUFFER_FULL;
        }
    }
    if (st == VMCU_OK) {
        uint8_t payload[24];
        vmcu_wbuf_t w;
        vmcu_wbuf_init(&w, payload, sizeof payload);
        header(&w, bus, cs_port, cs_pin, XFER_BULK, len);
        vmcu_wbuf_u16(&w, vmcu__stream_id(bulk));
        vmcu_wbuf_u16(&w, 0);
        vmcu_wbuf_u64(&w, position);
        st = vmcu__send(VMCU_MSG_SPI_XFER, payload, w.len);
    }
    vmcu_plat_mutex_unlock(bulk_mx);
    return st;
}

vmcu_status_t vmcu_spi_xfer(uint8_t bus, uint8_t cs_port, uint8_t cs_pin, const uint8_t *tx,
                            uint8_t *rx, size_t len, uint32_t timeout_ms)
{
    if (len == 0) {
        return VMCU_OK;
    }
    vmcu_status_t st = vmcu__ensure_init();
    if (st != VMCU_OK) {
        return st;
    }
    vmcu__checkpoint();
    if (rx != NULL) {
        for (size_t done = 0; done < len && st == VMCU_OK; done += VMCU_SPI_INLINE_MAX) {
            size_t n = len - done < VMCU_SPI_INLINE_MAX ? len - done : VMCU_SPI_INLINE_MAX;
            st = xfer_sync(bus, cs_port, cs_pin, tx != NULL ? tx + done : NULL, rx + done, n,
                           timeout_ms);
        }
        return st;
    }
    if (len <= VMCU_SPI_INLINE_MAX || tx == NULL) {
        for (size_t done = 0; done < len && st == VMCU_OK; done += VMCU_SPI_INLINE_MAX) {
            size_t n = len - done < VMCU_SPI_INLINE_MAX ? len - done : VMCU_SPI_INLINE_MAX;
            st = write_inline(bus, cs_port, cs_pin, tx != NULL ? tx + done : NULL, n);
        }
        return st;
    }
    return write_bulk(bus, cs_port, cs_pin, tx, len);
}
