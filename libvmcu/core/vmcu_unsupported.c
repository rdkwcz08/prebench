/*
 * libvmcu/core/vmcu_unsupported.c — Familles de l'API v1 pas encore simulées.
 *
 * Phase 1 : GPIO, temps, interruptions et ping seulement. Les fonctions
 * ci-dessous existent pour que tout firmware se lie (règle 16.2.10) ; elles
 * avertissent une fois et renvoient VMCU_ERR_UNSUPPORTED.
 * PWM et ADC arrivent en phase 2, UART en phase 2, I2C, SPI et flux en phase 3
 * (cahier des charges, section 14) : chaque fonction quittera ce fichier.
 */
#include "vmcu_internal.h"

static vmcu_status_t unsupported(const char *name)
{
    vmcu_warn_unsimulated(name);
    return VMCU_ERR_UNSUPPORTED;
}

/* --- PWM (phase 2) --- */

vmcu_status_t vmcu_pwm_config(uint8_t unit, uint8_t channel, uint32_t freq_hz)
{
    (void)unit; (void)channel; (void)freq_hz;
    return unsupported("vmcu_pwm_config");
}

vmcu_status_t vmcu_pwm_set_duty(uint8_t unit, uint8_t channel, uint16_t duty)
{
    (void)unit; (void)channel; (void)duty;
    return unsupported("vmcu_pwm_set_duty");
}

vmcu_status_t vmcu_pwm_attach(uint8_t unit, uint8_t channel, uint8_t port, uint8_t pin)
{
    (void)unit; (void)channel; (void)port; (void)pin;
    return unsupported("vmcu_pwm_attach");
}

/* --- ADC (phase 2) --- */

vmcu_status_t vmcu_adc_config(uint8_t unit, uint8_t channel)
{
    (void)unit; (void)channel;
    return unsupported("vmcu_adc_config");
}

uint16_t vmcu_adc_read(uint8_t unit, uint8_t channel)
{
    (void)unit; (void)channel;
    (void)unsupported("vmcu_adc_read");
    return 0;
}

/* --- UART (phase 2) --- */

vmcu_status_t vmcu_uart_config(uint8_t id, uint32_t baud)
{
    (void)id; (void)baud;
    return unsupported("vmcu_uart_config");
}

vmcu_status_t vmcu_uart_write(uint8_t id, const uint8_t *data, size_t len)
{
    (void)id; (void)data; (void)len;
    return unsupported("vmcu_uart_write");
}

size_t vmcu_uart_read(uint8_t id, uint8_t *buf, size_t max, uint32_t timeout_ms)
{
    (void)id; (void)buf; (void)max; (void)timeout_ms;
    (void)unsupported("vmcu_uart_read");
    return 0;
}

size_t vmcu_uart_available(uint8_t id)
{
    (void)id;
    (void)unsupported("vmcu_uart_available");
    return 0;
}

vmcu_status_t vmcu_uart_on_rx(uint8_t id, vmcu_uart_rx_cb cb, void *user)
{
    (void)id; (void)cb; (void)user;
    return unsupported("vmcu_uart_on_rx");
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
