/*
 * libvmcu/core/vmcu_adc.c — ADC (vmcu.h ; docs/protocol.md, ADC_CONFIG, ADC_VALUE).
 *
 * Les lectures sont servies par un cache local, sans aller-retour : l'app
 * envoie la valeur brute de chaque canal à la poignée de main (avant WELCOME),
 * puis à chaque changement de tension sur la broche (potentiomètre, source
 * analogique). La résolution est celle de la carte (adc_bits, reçue dans
 * WELCOME) ; c'est l'app qui convertit la tension avec la tension de
 * référence du fichier de carte.
 */
#include "vmcu_internal.h"

#include <stdatomic.h>

#define ADC_UNITS    4u  /* STM32 : ADC1..ADC3 ; ESP32 : ADC1, ADC2 */
#define ADC_CHANNELS 32u

static atomic_ushort values[ADC_UNITS][ADC_CHANNELS];

/* Dernier canal lu par ce fil, pour détecter l'attente active. */
static _Thread_local int t_last_key = -1;
static _Thread_local uint16_t t_last_value;

vmcu_status_t vmcu_adc_config(uint8_t unit, uint8_t channel)
{
    if (unit >= ADC_UNITS || channel >= ADC_CHANNELS) {
        return VMCU_ERR_INVALID_ARG;
    }
    vmcu_status_t st = vmcu__ensure_init();
    if (st != VMCU_OK) {
        return st;
    }
    vmcu__checkpoint();
    vmcu__guard_call();
    uint8_t payload[2] = { unit, channel };
    return vmcu__send(VMCU_MSG_ADC_CONFIG, payload, sizeof payload);
}

uint16_t vmcu_adc_read(uint8_t unit, uint8_t channel)
{
    if (unit >= ADC_UNITS || channel >= ADC_CHANNELS || vmcu__ensure_init() != VMCU_OK) {
        return 0;
    }
    vmcu__checkpoint();
    vmcu__guard_call();
    uint16_t v = atomic_load(&values[unit][channel]);
    int key = (int)unit * (int)ADC_CHANNELS + (int)channel;
    /* Lectures répétées d'une valeur figée : micro-pauses (section 5.5). */
    vmcu__guard_poll(key != t_last_key || v != t_last_value);
    t_last_key = key;
    t_last_value = v;
    return v;
}

/* Fil E/S : ADC_VALUE, valeur bornée à la résolution de la carte. */
void vmcu__adc_on_value(uint8_t unit, uint8_t channel, uint16_t raw)
{
    if (unit >= ADC_UNITS || channel >= ADC_CHANNELS) {
        return;
    }
    uint8_t bits = vmcu__adc_bits();
    if (bits > 0 && bits < 16) {
        uint16_t max = (uint16_t)((1u << bits) - 1u);
        if (raw > max) {
            raw = max;
        }
    }
    atomic_store(&values[unit][channel], raw);
}
