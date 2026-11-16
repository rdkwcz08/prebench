/*
 * libvmcu/core/vmcu_unsupported.c — Familles de l'API v1 pas encore simulées.
 *
 * GPIO, temps, UART, PWM, ADC (phases 1 et 2) et I2C (phase 3) sont simulés. Les fonctions
 * ci-dessous existent pour que tout firmware se lie (règle 16.2.10) ; elles
 * avertissent une fois et renvoient VMCU_ERR_UNSUPPORTED. SPI, flux et LED
 * adressables arrivent en phase 3 (cahier des charges, section 14) :
 * chaque fonction quittera ce fichier.
 */
#include "vmcu_internal.h"

static vmcu_status_t unsupported(const char *name)
{
    vmcu_warn_unsimulated(name);
    return VMCU_ERR_UNSUPPORTED;
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

/* --- LED adressables (phase 3, étape 8 ; API 1.2) --- */

vmcu_status_t vmcu_led_strip_show(uint8_t port, uint8_t pin, const uint8_t *data, size_t len)
{
    (void)port; (void)pin; (void)data; (void)len;
    return unsupported("vmcu_led_strip_show");
}

vmcu_status_t vmcu_led_strip_show_pwm(uint8_t unit, uint8_t channel, const uint8_t *data,
                                      size_t len)
{
    (void)unit; (void)channel; (void)data; (void)len;
    return unsupported("vmcu_led_strip_show_pwm");
}
