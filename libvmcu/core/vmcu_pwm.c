/*
 * libvmcu/core/vmcu_pwm.c — PWM (vmcu.h ; docs/protocol.md, PWM_CONFIG, PWM_DUTY, PWM_ATTACH).
 *
 * On simule le rapport cyclique, pas le signal (cahier des charges, section 8.2) :
 * seuls la fréquence et le rapport cyclique transitent, jamais les fronts. Une
 * PWM à 20 kHz ne coûte donc rien tant que le firmware ne change pas sa
 * consigne.
 *
 * Un firmware qui change la consigne très souvent (fondu, asservissement) ne
 * peut pas saturer le canal : PWM_DUTY est une trame remplaçable, et le canal
 * porte au plus une valeur par canal PWM et par lot (1 ms). Une consigne
 * identique à la précédente n'est pas renvoyée.
 */
#include "vmcu_internal.h"

#define PWM_UNITS    16u /* STM32 : TIM1..TIM14 ; ESP32 : timers LEDC */
#define PWM_CHANNELS 16u /* STM32 : 1..4 ; ESP32 : canaux LEDC 0..15 */

typedef struct {
    bool           configured;
    bool           duty_known;
    uint32_t       freq_hz;
    uint16_t       duty;
    vmcu__latest_t pending; /* PWM_DUTY en attente dans le lot */
} pwm_channel_t;

static pwm_channel_t channels[PWM_UNITS][PWM_CHANNELS];
static vmcu_plat_mutex_t *pwm_mx;

static bool valid(uint8_t unit, uint8_t channel)
{
    return unit < PWM_UNITS && channel < PWM_CHANNELS;
}

/* Création paresseuse du verrou : premier appel PWM, après vmcu_init. */
static vmcu_status_t prepare(uint8_t unit, uint8_t channel)
{
    if (!valid(unit, channel)) {
        return VMCU_ERR_INVALID_ARG;
    }
    vmcu_status_t st = vmcu__ensure_init();
    if (st != VMCU_OK) {
        return st;
    }
    vmcu__checkpoint();
    vmcu__guard_call();
    return VMCU_OK;
}

void vmcu__pwm_init(void)
{
    pwm_mx = vmcu_plat_mutex_create(false);
}

vmcu_status_t vmcu_pwm_config(uint8_t unit, uint8_t channel, uint32_t freq_hz)
{
    vmcu_status_t st = prepare(unit, channel);
    if (st != VMCU_OK) {
        return st;
    }
    vmcu_plat_mutex_lock(pwm_mx);
    pwm_channel_t *c = &channels[unit][channel];
    bool same = c->configured && c->freq_hz == freq_hz;
    c->configured = true;
    c->freq_hz = freq_hz;
    if (!same) {
        /* La consigne suivante part après cette configuration, jamais avant. */
        vmcu__forget_latest(&c->pending);
    }
    vmcu_plat_mutex_unlock(pwm_mx);
    if (same) {
        return VMCU_OK;
    }
    uint8_t payload[8];
    vmcu_wbuf_t w;
    vmcu_wbuf_init(&w, payload, sizeof payload);
    vmcu_wbuf_u8(&w, unit);
    vmcu_wbuf_u8(&w, channel);
    vmcu_wbuf_u16(&w, 0);
    vmcu_wbuf_u32(&w, freq_hz);
    return vmcu__send(VMCU_MSG_PWM_CONFIG, payload, w.len);
}

vmcu_status_t vmcu_pwm_set_duty(uint8_t unit, uint8_t channel, uint16_t duty)
{
    vmcu_status_t st = prepare(unit, channel);
    if (st != VMCU_OK) {
        return st;
    }
    uint8_t payload[4];
    vmcu_wbuf_t w;
    vmcu_wbuf_init(&w, payload, sizeof payload);
    vmcu_wbuf_u8(&w, unit);
    vmcu_wbuf_u8(&w, channel);
    vmcu_wbuf_u16(&w, duty);
    vmcu_plat_mutex_lock(pwm_mx);
    pwm_channel_t *c = &channels[unit][channel];
    if (c->duty_known && c->duty == duty) {
        vmcu_plat_mutex_unlock(pwm_mx);
        return VMCU_OK;
    }
    c->duty_known = true;
    c->duty = duty;
    /* Verrou PWM tenu : deux consignes du même canal ne se croisent pas. */
    st = vmcu__send_latest(VMCU_MSG_PWM_DUTY, payload, w.len, &c->pending);
    vmcu_plat_mutex_unlock(pwm_mx);
    return st;
}

vmcu_status_t vmcu_pwm_attach(uint8_t unit, uint8_t channel, uint8_t port, uint8_t pin)
{
    vmcu_status_t st = prepare(unit, channel);
    if (st != VMCU_OK) {
        return st;
    }
    if (port >= VMCU_GPIO_MAX_PORTS || pin >= VMCU_GPIO_MAX_PINS) {
        return VMCU_ERR_INVALID_ARG;
    }
    vmcu_plat_mutex_lock(pwm_mx);
    vmcu__forget_latest(&channels[unit][channel].pending);
    vmcu_plat_mutex_unlock(pwm_mx);
    uint8_t payload[4] = { unit, channel, port, pin };
    return vmcu__send(VMCU_MSG_PWM_ATTACH, payload, sizeof payload);
}
