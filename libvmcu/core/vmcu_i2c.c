/*
 * libvmcu/core/vmcu_i2c.c — I2C maître (vmcu.h ; docs/protocol.md, I2C_CONFIG,
 * I2C_XFER, I2C_REPLY ; phase 3, étape 2).
 *
 * Une transaction = une trame I2C_XFER synchrone : le firmware attend la
 * réponse du modèle du composant, comme sur un vrai bus (section 6.1). Sans
 * composant à l'adresse, l'app répond NACK adresse.
 */
#include "vmcu_internal.h"

#include <string.h>

vmcu_status_t vmcu_i2c_config(uint8_t bus, uint32_t freq_hz)
{
    vmcu_status_t st = vmcu__ensure_init();
    if (st != VMCU_OK) {
        return st;
    }
    uint8_t payload[8];
    vmcu_wbuf_t w;
    vmcu_wbuf_init(&w, payload, sizeof payload);
    vmcu_wbuf_u8(&w, bus);
    vmcu_wbuf_u8(&w, 0);
    vmcu_wbuf_u8(&w, 0);
    vmcu_wbuf_u8(&w, 0);
    vmcu_wbuf_u32(&w, freq_hz);
    return vmcu__send(VMCU_MSG_I2C_CONFIG, payload, w.len);
}

vmcu_status_t vmcu_i2c_xfer(uint8_t bus, uint8_t addr7, const uint8_t *tx, size_t txlen,
                            uint8_t *rx, size_t rxlen, uint32_t timeout_ms)
{
    if (addr7 > 0x7F || txlen > VMCU_I2C_MAX_XFER || rxlen > VMCU_I2C_MAX_XFER
        || (tx == NULL && txlen > 0) || (rx == NULL && rxlen > 0)) {
        return VMCU_ERR_INVALID_ARG;
    }
    vmcu_status_t st = vmcu__ensure_init();
    if (st != VMCU_OK) {
        return st;
    }
    vmcu__checkpoint();

    uint8_t payload[8 + VMCU_I2C_MAX_XFER];
    vmcu_wbuf_t w;
    vmcu_wbuf_init(&w, payload, sizeof payload);
    vmcu_wbuf_u8(&w, bus);
    vmcu_wbuf_u8(&w, addr7);
    vmcu_wbuf_u16(&w, (uint16_t)txlen);
    vmcu_wbuf_u16(&w, (uint16_t)rxlen);
    vmcu_wbuf_u16(&w, 0);
    if (txlen > 0) {
        vmcu_wbuf_bytes(&w, tx, txlen);
    }

    uint8_t reply[4 + VMCU_I2C_MAX_XFER];
    size_t reply_len = 0;
    uint16_t reply_type = 0;
    st = vmcu__request(VMCU_MSG_I2C_XFER, payload, w.len, reply, sizeof reply, &reply_len,
                       &reply_type, timeout_ms);
    if (st != VMCU_OK) {
        return st;
    }
    if (reply_type != VMCU_MSG_I2C_REPLY || reply_len < 4) {
        return VMCU_ERR_PROTOCOL;
    }
    vmcu_rbuf_t r;
    vmcu_rbuf_init(&r, reply, reply_len);
    uint8_t status = vmcu_rbuf_u8(&r);
    (void)vmcu_rbuf_u8(&r);
    size_t got = vmcu_rbuf_u16(&r);
    switch (status) {
    case 0:
        break;
    case 1:
    case 2:
        return VMCU_ERR_NACK;
    default:
        return VMCU_ERR_PROTOCOL; /* erreur de bus signalée par l'app */
    }
    if (rxlen > 0) {
        size_t avail = reply_len - 4;
        size_t n = got < avail ? got : avail;
        if (n > rxlen) {
            n = rxlen;
        }
        memcpy(rx, reply + 4, n);
        if (n < rxlen) {
            memset(rx + n, 0xFF, rxlen - n); /* le cœur complète déjà ; défense en profondeur */
        }
    }
    return VMCU_OK;
}
