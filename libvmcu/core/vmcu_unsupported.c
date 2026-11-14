/*
 * libvmcu/core/vmcu_unsupported.c — Familles de l'API v1 pas encore simulées.
 *
 * GPIO, temps, UART, PWM et ADC sont simulés (phases 1 et 2). Les fonctions
 * ci-dessous existent pour que tout firmware se lie (règle 16.2.10) ; elles
 * avertissent une fois et renvoient VMCU_ERR_UNSUPPORTED. I2C, SPI et flux
 * arrivent en phase 3 (cahier des charges, section 14) : chaque fonction
 * quittera ce fichier.
 */
#include "vmcu_internal.h"

static vmcu_status_t unsupported(const char *name)
{
    vmcu_warn_unsimulated(name);
    return VMCU_ERR_UNSUPPORTED;
}

/* --- I2C (phase 3) --- */

vmcu_status_t vmcu_i2c_config(uint8_t bus, uint32_t freq_hz)
{
    (void)bus; (void)freq_hz;
    return unsupported("vmcu_i2c_config");
}

vmcu_status_t vmcu_i2c_xfer(uint8_t bus, uint8_t addr7, const uint8_t *tx, size_t txlen,
                            uint8_t *rx, size_t rxlen, uint32_t timeout_ms)
{
    (void)bus; (void)addr7; (void)tx; (void)txlen; (void)rx; (void)rxlen; (void)timeout_ms;
    return unsupported("vmcu_i2c_xfer");
}

/* --- SPI (phase 3) --- */

vmcu_status_t vmcu_spi_config(uint8_t bus, vmcu_spi_mode_t mode, uint32_t freq_hz, bool lsb_first)
{
    (void)bus; (void)mode; (void)freq_hz; (void)lsb_first;
    return unsupported("vmcu_spi_config");
}

vmcu_status_t vmcu_spi_xfer(uint8_t bus, uint8_t cs_port, uint8_t cs_pin, const uint8_t *tx,
                            uint8_t *rx, size_t len, uint32_t timeout_ms)
{
    (void)bus; (void)cs_port; (void)cs_pin; (void)tx; (void)rx; (void)len; (void)timeout_ms;
    return unsupported("vmcu_spi_xfer");
}

/* --- Flux en mémoire partagée (phase 3) --- */

vmcu_status_t vmcu_stream_open(vmcu_stream_kind_t kind, const vmcu_stream_params_t *params,
                               vmcu_stream_t **out)
{
    (void)kind; (void)params;
    if (out != NULL) {
        *out = NULL;
    }
    return unsupported("vmcu_stream_open");
}

void *vmcu_stream_data(vmcu_stream_t *stream)
{
    (void)stream;
    return NULL;
}

size_t vmcu_stream_capacity(const vmcu_stream_t *stream)
{
    (void)stream;
    return 0;
}

size_t vmcu_stream_write(vmcu_stream_t *stream, const void *data, size_t len)
{
    (void)stream; (void)data; (void)len;
    return 0;
}

size_t vmcu_stream_read(vmcu_stream_t *stream, void *buf, size_t len)
{
    (void)stream; (void)buf; (void)len;
    return 0;
}

void vmcu_stream_commit(vmcu_stream_t *stream)
{
    (void)stream;
}

void vmcu_stream_close(vmcu_stream_t *stream)
{
    (void)stream;
}
