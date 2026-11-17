/*
 * libvmcu/tests/test_ws2812.c — Ruban WS2812 piloté par HAL_TIM_PWM_Start_DMA
 * (phase 3, étape 8), face à une fausse app en protocole 1.2 :
 * décodage du tampon DMA (demi-mots, impulsion longue = 1, courte = 0, repos
 * ignoré), trame LED_STRIP avec l'unité et le canal du timer, callbacks
 * demi-transfert puis fin depuis le fil IRQ, canal occupé jusqu'à
 * HAL_TIM_PWM_Stop_DMA, trame identique non renvoyée, trame trop longue
 * refusée. Compilé aussi avec PREBENCH_TEST_PROTO11 : face à une app 1.1,
 * rien n'est envoyé et vmcu_led_strip_show renvoie VMCU_ERR_UNSUPPORTED.
 */
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "fake_app.h"
#include "stm32f4xx_hal.h"
#include "vmcu_platform.h"
#include "vmcu_wire.h"

#define T1H 67u /* 84 MHz / 105 = 800 kHz : 1 ≈ 0,8 µs */
#define T0H 34u /* 0 ≈ 0,4 µs */
#define LEDS 2u
#define RESET_SLOTS 50u

#ifndef PREBENCH_TEST_PROTO11
static TIM_HandleTypeDef htim1;
static DMA_HandleTypeDef hdma_tim1_ch1;
static atomic_int half;
static atomic_int done;

void HAL_TIM_PWM_PulseFinishedHalfCpltCallback(TIM_HandleTypeDef *htim)
{
    if (htim == &htim1) {
        atomic_fetch_add(&half, 1);
    }
}

void HAL_TIM_PWM_PulseFinishedCallback(TIM_HandleTypeDef *htim)
{
    if (htim == &htim1 && htim->Channel == HAL_TIM_ACTIVE_CHANNEL_1) {
        atomic_fetch_add(&done, 1);
    }
}

static bool wait_for(atomic_int *v, int expected)
{
    for (int t = 0; t < 200; ++t) {
        if (atomic_load(v) >= expected) {
            return true;
        }
        vmcu_plat_sleep_ns(5000000ull);
    }
    return false;
}

/* Tampon DMA d'une trame GRB : un demi-mot par bit, puis le repos. */
static uint16_t dma_buf[LEDS * 24u + RESET_SLOTS];

static void encode(const uint8_t *grb, size_t n)
{
    size_t k = 0;
    for (size_t i = 0; i < n; ++i) {
        for (int b = 7; b >= 0; --b) {
            dma_buf[k++] = ((grb[i] >> b) & 1u) ? T1H : T0H;
        }
    }
    while (k < sizeof dma_buf / sizeof dma_buf[0]) {
        dma_buf[k++] = 0;
    }
}
#endif

int main(void)
{
    fake_app_t *fa = fake_app_create(1);
#ifdef PREBENCH_TEST_PROTO11
    fake_app_set_protocol_minor(fa, 1);
#endif
    fake_app_run(fa);
    CHECK(HAL_Init() == HAL_OK);

#ifdef PREBENCH_TEST_PROTO11
    const uint8_t one[3] = { 1, 2, 3 };
    CHECK(vmcu_led_strip_show(0, 8, one, sizeof one) == VMCU_ERR_UNSUPPORTED);
    CHECK(vmcu_ping(VMCU_PING_CORE, NULL) == VMCU_OK);
    CHECK(fake_app_count(fa, VMCU_MSG_LED_STRIP, -1, -1) == 0);
    CHECK(fake_app_wait(fa, VMCU_MSG_WARN, -1, -1, 1, 1000, NULL));
    vmcu_shutdown();
    fake_app_stop(fa);
    printf("test_ws2812 (app 1.1) : OK\n");
    return 0;
#else
    /* TIM1 à 800 kHz, canal 1 en PWM1, DMA lié en demi-mots (comme CubeMX). */
    htim1.Instance = TIM1;
    htim1.Init.Prescaler = 0;
    htim1.Init.Period = 104;
    htim1.Init.CounterMode = TIM_COUNTERMODE_UP;
    CHECK(HAL_TIM_PWM_Init(&htim1) == HAL_OK);
    TIM_OC_InitTypeDef oc = { 0 };
    oc.OCMode = TIM_OCMODE_PWM1;
    oc.OCPolarity = TIM_OCPOLARITY_HIGH;
    CHECK(HAL_TIM_PWM_ConfigChannel(&htim1, &oc, TIM_CHANNEL_1) == HAL_OK);
    hdma_tim1_ch1.Init.MemDataAlignment = DMA_MDATAALIGN_HALFWORD;
    hdma_tim1_ch1.Init.Mode = DMA_NORMAL;
    __HAL_LINKDMA(&htim1, hdma[TIM_DMA_ID_CC1], hdma_tim1_ch1);

    /* Deux LED : (G 0x10, R 0x20, B 0x30) puis (0xFF, 0x00, 0x81). */
    const uint8_t grb[6] = { 0x10, 0x20, 0x30, 0xFF, 0x00, 0x81 };
    encode(grb, sizeof grb);
    CHECK(HAL_TIM_PWM_Start_DMA(&htim1, TIM_CHANNEL_1, (const uint32_t *)dma_buf,
                                (uint16_t)(sizeof dma_buf / sizeof dma_buf[0])) == HAL_OK);
    fake_frame_t f;
    CHECK(fake_app_wait(fa, VMCU_MSG_LED_STRIP, -1, -1, 1, 1000, &f));
    CHECK(f.data[0] == 0xFF && f.data[1] == 0xFF && f.data[2] == 1 && f.data[3] == 1);
    CHECK(vmcu_wire_get_u16(f.data + 4) == sizeof grb);
    CHECK(memcmp(f.data + 8, grb, sizeof grb) == 0);
    CHECK(wait_for(&done, 1));
    CHECK(atomic_load(&half) == 1);

    /* Canal occupé tant que le firmware n'a pas appelé Stop_DMA, comme sur la puce. */
    CHECK(HAL_TIM_PWM_Start_DMA(&htim1, TIM_CHANNEL_1, (const uint32_t *)dma_buf, 10) == HAL_BUSY);
    CHECK(HAL_TIM_PWM_Stop_DMA(&htim1, TIM_CHANNEL_1) == HAL_OK);

    /* Même trame : rien de plus sur le canal. */
    CHECK(HAL_TIM_PWM_Start_DMA(&htim1, TIM_CHANNEL_1, (const uint32_t *)dma_buf,
                                (uint16_t)(sizeof dma_buf / sizeof dma_buf[0])) == HAL_OK);
    CHECK(wait_for(&done, 2));
    CHECK(HAL_TIM_PWM_Stop_DMA(&htim1, TIM_CHANNEL_1) == HAL_OK);
    CHECK(vmcu_ping(VMCU_PING_CORE, NULL) == VMCU_OK);
    CHECK(fake_app_count(fa, VMCU_MSG_LED_STRIP, -1, -1) == 1);

    /* Autre trame : envoyée. */
    const uint8_t next[6] = { 0x00, 0x00, 0x00, 0x01, 0x02, 0x03 };
    encode(next, sizeof next);
    CHECK(HAL_TIM_PWM_Start_DMA(&htim1, TIM_CHANNEL_1, (const uint32_t *)dma_buf,
                                (uint16_t)(sizeof dma_buf / sizeof dma_buf[0])) == HAL_OK);
    CHECK(fake_app_wait(fa, VMCU_MSG_LED_STRIP, -1, -1, 2, 1000, &f));
    CHECK(memcmp(f.data + 8, next, sizeof next) == 0);
    CHECK(wait_for(&done, 3));
    CHECK(HAL_TIM_PWM_Stop_DMA(&htim1, TIM_CHANNEL_1) == HAL_OK);

    /* Par broche (ESP32, bibliothèques) ; au-delà de 4096 octets : refusé. */
    static uint8_t big[VMCU_LED_STRIP_MAX_BYTES + 1];
    CHECK(vmcu_led_strip_show(1, 6, big, 3) == VMCU_OK);
    CHECK(fake_app_wait(fa, VMCU_MSG_LED_STRIP, 1, 6, 1, 1000, &f));
    CHECK(f.data[2] == 0xFF && f.data[3] == 0xFF);
    CHECK(vmcu_led_strip_show(1, 6, big, sizeof big) == VMCU_ERR_INVALID_ARG);

    vmcu_shutdown();
    fake_app_stop(fa);
    printf("test_ws2812 : OK\n");
    return 0;
#endif
}
