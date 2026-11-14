/*
 * libvmcu/tests/test_pwm_adc.c — PWM des timers et ADC du HAL STM32 sur libvmcu,
 * face à une fausse app (phase 2, étape 8) :
 * - PWM à 20 kHz : seules fréquence et rapport cyclique transitent ; 20 000
 *   consignes en rafale donnent au plus une trame par lot (1 ms), la dernière
 *   valeur gagne ;
 * - horloge des timers déduite de HAL_RCC_OscConfig et HAL_RCC_ClockConfig
 *   (16 MHz sans configuration, 84 MHz avec celle de CubeMX, autre réglage) ;
 * - PWM2 et polarité basse inversent le rapport ; prescaler et période
 *   recalculent la fréquence ; Stop envoie une fréquence nulle ;
 * - ADC : valeur reçue avant WELCOME et avant ADC_CONFIG conservée, valeur
 *   reçue ensuite (ADC_VALUE), résolution et alignement du HAL, séquence
 *   en mode scan, Start_IT et son callback, DMA qui avertit « non simulé ».
 */
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "fake_app.h"
#include "stm32f4xx_hal.h"
#include "vmcu_platform.h"
#include "vmcu_wire.h"

static TIM_HandleTypeDef htim2;
static ADC_HandleTypeDef hadc1;
static atomic_int conv_done;

void HAL_ADC_ConvCpltCallback(ADC_HandleTypeDef *hadc)
{
    if (hadc == &hadc1) {
        atomic_fetch_add(&conv_done, 1);
    }
}

/* SystemClock_Config de CubeMX pour la Nucleo-F401RE : HSI 16 MHz, PLL à 84 MHz,
 * APB1 divisé par 2 (timers à 84 MHz), APB2 non divisé. */
static void clock_84mhz(void)
{
    RCC_OscInitTypeDef osc = {0};
    osc.OscillatorType = RCC_OSCILLATORTYPE_HSI;
    osc.HSIState = RCC_HSI_ON;
    osc.PLL.PLLState = RCC_PLL_ON;
    osc.PLL.PLLSource = RCC_PLLSOURCE_HSI;
    osc.PLL.PLLM = 16;
    osc.PLL.PLLN = 336;
    osc.PLL.PLLP = RCC_PLLP_DIV4;
    osc.PLL.PLLQ = 7;
    CHECK(HAL_RCC_OscConfig(&osc) == HAL_OK);
    RCC_ClkInitTypeDef clk = {0};
    clk.ClockType = RCC_CLOCKTYPE_HCLK | RCC_CLOCKTYPE_SYSCLK | RCC_CLOCKTYPE_PCLK1 | RCC_CLOCKTYPE_PCLK2;
    clk.SYSCLKSource = RCC_SYSCLKSOURCE_PLLCLK;
    clk.AHBCLKDivider = RCC_SYSCLK_DIV1;
    clk.APB1CLKDivider = RCC_HCLK_DIV2;
    clk.APB2CLKDivider = RCC_HCLK_DIV1;
    CHECK(HAL_RCC_ClockConfig(&clk, FLASH_LATENCY_2) == HAL_OK);
}

static uint16_t duty_of(const fake_frame_t *f)
{
    return vmcu_wire_get_u16(f->data + 2);
}

static void config_channel(uint32_t mode, uint32_t polarity, uint32_t pulse)
{
    TIM_OC_InitTypeDef oc = {0};
    oc.OCMode = mode;
    oc.Pulse = pulse;
    oc.OCPolarity = polarity;
    CHECK(HAL_TIM_PWM_ConfigChannel(&htim2, &oc, TIM_CHANNEL_1) == HAL_OK);
}

static bool wait_adc(uint8_t channel, uint16_t expected)
{
    for (int t = 0; t < 200; ++t) {
        if (vmcu_adc_read(1, channel) == expected) {
            return true;
        }
        vmcu_plat_sleep_ns(5000000ull);
    }
    return false;
}

int main(void)
{
    fake_app_t *fa = fake_app_create(1);
    /* Valeur envoyée avant WELCOME, avant toute configuration de l'ADC : conservée. */
    fake_app_initial_adc(fa, 1, 1, 3000);
    fake_app_run(fa);
    CHECK(HAL_Init() == HAL_OK);
    CHECK(vmcu_adc_read(1, 1) == 3000U);
    fake_frame_t f;

    /* --- Horloge déduite de la configuration RCC -------------------------------------- */
    /* Sans SystemClock_Config, comme la puce : HSI à 16 MHz. */
    CHECK(HAL_RCC_GetSysClockFreq() == 16000000U && SystemCoreClock == 16000000U);
    clock_84mhz();
    CHECK(HAL_RCC_GetSysClockFreq() == 84000000U && SystemCoreClock == 84000000U);
    CHECK(HAL_RCC_GetPCLK1Freq() == 42000000U && HAL_RCC_GetPCLK2Freq() == 84000000U);

    /* --- PWM à 20 kHz : 84 MHz / (1 × 4200) ------------------------------------------- */
    htim2.Instance = TIM2;
    htim2.Init.Prescaler = 0;
    htim2.Init.CounterMode = TIM_COUNTERMODE_UP;
    htim2.Init.Period = 4199;
    CHECK(HAL_TIM_PWM_Init(&htim2) == HAL_OK);
    config_channel(TIM_OCMODE_PWM1, TIM_OCPOLARITY_HIGH, 2100);
    CHECK(HAL_TIM_PWM_Start(&htim2, TIM_CHANNEL_1) == HAL_OK);
    CHECK(fake_app_wait(fa, VMCU_MSG_PWM_CONFIG, 2, 1, 1, 1000, &f));
    CHECK(vmcu_wire_get_u32(f.data + 4) == 20000U);
    CHECK(fake_app_wait(fa, VMCU_MSG_PWM_DUTY, 2, 1, 1, 1000, &f));
    CHECK(duty_of(&f) == 32768U); /* 50 % */

    /* Une PWM qui tourne n'émet rien : aucun front ne transite. */
    unsigned before = fake_app_count(fa, VMCU_MSG_PWM_DUTY, 2, 1);
    HAL_Delay(50);
    CHECK(fake_app_count(fa, VMCU_MSG_PWM_DUTY, 2, 1) == before);

    /* 20 000 consignes en rafale : au plus une trame par lot, la dernière gagne. */
    for (uint32_t i = 0; i < 20000U; ++i) {
        __HAL_TIM_SET_COMPARE(&htim2, TIM_CHANNEL_1, i % 4200U);
    }
    __HAL_TIM_SET_COMPARE(&htim2, TIM_CHANNEL_1, 1050); /* 25 % */
    HAL_Delay(5);
    CHECK(fake_app_wait(fa, VMCU_MSG_PWM_DUTY, 2, 1, before + 1, 1000, &f));
    unsigned burst = fake_app_count(fa, VMCU_MSG_PWM_DUTY, 2, 1) - before;
    printf("test_pwm_adc : 20 001 consignes -> %u trames PWM_DUTY\n", burst);
    CHECK(burst <= 200U);
    HAL_Delay(5);
    CHECK(fake_app_wait(fa, VMCU_MSG_PWM_DUTY, 2, 1, fake_app_count(fa, VMCU_MSG_PWM_DUTY, 2, 1), 1000, &f));
    CHECK(duty_of(&f) == 16384U);

    /* PWM2 et polarité basse : inversions qui s'annulent ; PWM2 seul inverse. */
    config_channel(TIM_OCMODE_PWM2, TIM_OCPOLARITY_LOW, 1050);
    HAL_Delay(5);
    CHECK(fake_app_wait(fa, VMCU_MSG_PWM_DUTY, 2, 1, fake_app_count(fa, VMCU_MSG_PWM_DUTY, 2, 1), 1000, &f));
    CHECK(duty_of(&f) == 16384U);
    config_channel(TIM_OCMODE_PWM2, TIM_OCPOLARITY_HIGH, 1050);
    HAL_Delay(5);
    CHECK(fake_app_wait(fa, VMCU_MSG_PWM_DUTY, 2, 1, fake_app_count(fa, VMCU_MSG_PWM_DUTY, 2, 1), 1000, &f));
    CHECK(duty_of(&f) == 49151U);

    /* Prescaler modifié en marche : 10 kHz ; Stop : fréquence nulle. */
    __HAL_TIM_SET_PRESCALER(&htim2, 1);
    CHECK(fake_app_wait(fa, VMCU_MSG_PWM_CONFIG, 2, 1, 2, 1000, &f));
    CHECK(vmcu_wire_get_u32(f.data + 4) == 10000U);
    CHECK(HAL_TIM_PWM_Stop(&htim2, TIM_CHANNEL_1) == HAL_OK);
    CHECK(fake_app_wait(fa, VMCU_MSG_PWM_CONFIG, 2, 1, 3, 1000, &f));
    CHECK(vmcu_wire_get_u32(f.data + 4) == 0U);

    /* Autre configuration : PLL sur HSE 8 MHz à 64 MHz, AHB /2, APB1 /1 :
       timers d'APB1 à 32 MHz ; 32 MHz / (2 × 4200) = 3 810 Hz (arrondi). */
    RCC_OscInitTypeDef osc = {0};
    osc.PLL.PLLState = RCC_PLL_ON;
    osc.PLL.PLLSource = RCC_PLLSOURCE_HSE;
    osc.PLL.PLLM = 8;
    osc.PLL.PLLN = 256;
    osc.PLL.PLLP = RCC_PLLP_DIV4;
    CHECK(HAL_RCC_OscConfig(&osc) == HAL_OK);
    RCC_ClkInitTypeDef clk = {0};
    clk.ClockType = RCC_CLOCKTYPE_HCLK | RCC_CLOCKTYPE_SYSCLK | RCC_CLOCKTYPE_PCLK1 | RCC_CLOCKTYPE_PCLK2;
    clk.SYSCLKSource = RCC_SYSCLKSOURCE_PLLCLK;
    clk.AHBCLKDivider = RCC_SYSCLK_DIV2;
    clk.APB1CLKDivider = RCC_HCLK_DIV1;
    clk.APB2CLKDivider = RCC_HCLK_DIV1;
    CHECK(HAL_RCC_ClockConfig(&clk, FLASH_LATENCY_2) == HAL_OK);
    CHECK(SystemCoreClock == 32000000U);
    CHECK(HAL_TIM_PWM_Start(&htim2, TIM_CHANNEL_1) == HAL_OK);
    CHECK(fake_app_wait(fa, VMCU_MSG_PWM_CONFIG, 2, 1, 4, 1000, &f));
    CHECK(vmcu_wire_get_u32(f.data + 4) == 3810U);
    CHECK(HAL_TIM_PWM_Stop(&htim2, TIM_CHANNEL_1) == HAL_OK);
    clock_84mhz();

    /* --- ADC ------------------------------------------------------------------------- */
    hadc1.Instance = ADC1;
    hadc1.Init.Resolution = ADC_RESOLUTION_12B;
    hadc1.Init.DataAlign = ADC_DATAALIGN_RIGHT;
    hadc1.Init.NbrOfConversion = 1;
    CHECK(HAL_ADC_Init(&hadc1) == HAL_OK);
    ADC_ChannelConfTypeDef ch = {0};
    ch.Channel = ADC_CHANNEL_0;
    ch.Rank = 1;
    CHECK(HAL_ADC_ConfigChannel(&hadc1, &ch) == HAL_OK);
    CHECK(fake_app_wait(fa, VMCU_MSG_ADC_CONFIG, 1, 0, 1, 1000, &f));

    fake_app_send_adc(fa, 1, 0, 2048);
    fake_app_send_adc(fa, 1, 4, 1000);
    fake_app_send_adc(fa, 1, 8, 9999); /* au-delà de 12 bits : bornée */
    CHECK(wait_adc(0, 2048) && wait_adc(4, 1000) && wait_adc(8, 4095));
    CHECK(HAL_ADC_Start(&hadc1) == HAL_OK);
    CHECK(HAL_ADC_PollForConversion(&hadc1, 10) == HAL_OK);
    CHECK(__HAL_ADC_GET_FLAG(&hadc1, ADC_FLAG_EOC) == SET);
    CHECK(HAL_ADC_GetValue(&hadc1) == 2048U);

    /* Résolution 10 bits, puis alignement à gauche en 12 bits. */
    hadc1.Init.Resolution = ADC_RESOLUTION_10B;
    CHECK(HAL_ADC_GetValue(&hadc1) == 512U);
    hadc1.Init.Resolution = ADC_RESOLUTION_12B;
    hadc1.Init.DataAlign = ADC_DATAALIGN_LEFT;
    CHECK(HAL_ADC_GetValue(&hadc1) == 2048U << 4);
    hadc1.Init.DataAlign = ADC_DATAALIGN_RIGHT;

    /* Séquence en mode scan : rang 1 (CH0), rang 2 (CH4), puis rang 1. */
    hadc1.Init.ScanConvMode = ENABLE;
    hadc1.Init.NbrOfConversion = 2;
    ch.Channel = ADC_CHANNEL_4;
    ch.Rank = 2;
    CHECK(HAL_ADC_ConfigChannel(&hadc1, &ch) == HAL_OK);
    CHECK(HAL_ADC_Start(&hadc1) == HAL_OK);
    CHECK(HAL_ADC_GetValue(&hadc1) == 2048U);
    CHECK(HAL_ADC_GetValue(&hadc1) == 1000U);
    CHECK(HAL_ADC_GetValue(&hadc1) == 2048U);

    /* Start_IT : HAL_ADC_ConvCpltCallback depuis le fil d'interruption. */
    CHECK(HAL_ADC_Start_IT(&hadc1) == HAL_OK);
    for (int t = 0; t < 200 && atomic_load(&conv_done) == 0; ++t) {
        vmcu_plat_sleep_ns(5000000ull);
    }
    CHECK(atomic_load(&conv_done) == 1);

    /* DMA : compile et avertit « non simulé ». */
    uint32_t buf[2];
    unsigned warns = fake_app_count(fa, VMCU_MSG_WARN, -1, -1);
    CHECK(HAL_ADC_Start_DMA(&hadc1, buf, 2) == HAL_OK);
    CHECK(fake_app_wait(fa, VMCU_MSG_WARN, -1, -1, warns + 1, 1000, &f));
    CHECK(memcmp(f.data + 4, "HAL_ADC_Start_DMA", 17) == 0);

    vmcu_shutdown();
    fake_app_stop(fa);
    printf("test_pwm_adc : OK\n");
    return 0;
}
