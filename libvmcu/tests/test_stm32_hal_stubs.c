/*
 * libvmcu/tests/test_stm32_hal_stubs.c — Modules non simulés en phase 1 (règle 16.2.10).
 *
 * Les fonctions MX_*_Init et HAL_*_MspInit ci-dessous reprennent le code que
 * STM32CubeMX génère pour TIM1 (PWM complémentaire), TIM2 (PWM), ADC1, I2C1,
 * SPI1 et USART2 avec réception DMA. Le test vérifie qu'elles compilent, que
 * les MspInit du projet sont appelés, qu'aucun Init n'échoue, et que les
 * transferts se comportent comme décrit dans stm32_hal_periph.c.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "fake_app.h"
#include "stm32f4xx_hal.h"
#include "vmcu_wire.h"

TIM_HandleTypeDef htim1;
TIM_HandleTypeDef htim2;
ADC_HandleTypeDef hadc1;
I2C_HandleTypeDef hi2c1;
SPI_HandleTypeDef hspi1;
UART_HandleTypeDef huart2;
DMA_HandleTypeDef hdma_usart2_rx;

static int msp_calls;
static int errors;

void Error_Handler(void)
{
    errors++;
}

void HAL_TIM_MspPostInit(TIM_HandleTypeDef *htim);

/* --- Code de style CubeMX ------------------------------------------------------------- */

static void MX_DMA_Init(void)
{
    __HAL_RCC_DMA1_CLK_ENABLE();
    HAL_NVIC_SetPriority(DMA1_Stream5_IRQn, 0, 0);
    HAL_NVIC_EnableIRQ(DMA1_Stream5_IRQn);
}

static void MX_TIM1_Init(void)
{
    TIM_ClockConfigTypeDef sClockSourceConfig = {0};
    TIM_MasterConfigTypeDef sMasterConfig = {0};
    TIM_OC_InitTypeDef sConfigOC = {0};
    TIM_BreakDeadTimeConfigTypeDef sBreakDeadTimeConfig = {0};

    htim1.Instance = TIM1;
    htim1.Init.Prescaler = 0;
    htim1.Init.CounterMode = TIM_COUNTERMODE_UP;
    htim1.Init.Period = 65535;
    htim1.Init.ClockDivision = TIM_CLOCKDIVISION_DIV1;
    htim1.Init.RepetitionCounter = 0;
    htim1.Init.AutoReloadPreload = TIM_AUTORELOAD_PRELOAD_DISABLE;
    if (HAL_TIM_Base_Init(&htim1) != HAL_OK) { Error_Handler(); }
    sClockSourceConfig.ClockSource = TIM_CLOCKSOURCE_INTERNAL;
    if (HAL_TIM_ConfigClockSource(&htim1, &sClockSourceConfig) != HAL_OK) { Error_Handler(); }
    if (HAL_TIM_PWM_Init(&htim1) != HAL_OK) { Error_Handler(); }
    sMasterConfig.MasterOutputTrigger = TIM_TRGO_RESET;
    sMasterConfig.MasterSlaveMode = TIM_MASTERSLAVEMODE_DISABLE;
    if (HAL_TIMEx_MasterConfigSynchronization(&htim1, &sMasterConfig) != HAL_OK) { Error_Handler(); }
    sConfigOC.OCMode = TIM_OCMODE_PWM1;
    sConfigOC.Pulse = 0;
    sConfigOC.OCPolarity = TIM_OCPOLARITY_HIGH;
    sConfigOC.OCNPolarity = TIM_OCNPOLARITY_HIGH;
    sConfigOC.OCFastMode = TIM_OCFAST_DISABLE;
    sConfigOC.OCIdleState = TIM_OCIDLESTATE_RESET;
    sConfigOC.OCNIdleState = TIM_OCNIDLESTATE_RESET;
    if (HAL_TIM_PWM_ConfigChannel(&htim1, &sConfigOC, TIM_CHANNEL_1) != HAL_OK) { Error_Handler(); }
    sBreakDeadTimeConfig.OffStateRunMode = TIM_OSSR_DISABLE;
    sBreakDeadTimeConfig.OffStateIDLEMode = TIM_OSSI_DISABLE;
    sBreakDeadTimeConfig.LockLevel = TIM_LOCKLEVEL_OFF;
    sBreakDeadTimeConfig.DeadTime = 0;
    sBreakDeadTimeConfig.BreakState = TIM_BREAK_DISABLE;
    sBreakDeadTimeConfig.BreakPolarity = TIM_BREAKPOLARITY_HIGH;
    sBreakDeadTimeConfig.AutomaticOutput = TIM_AUTOMATICOUTPUT_DISABLE;
    if (HAL_TIMEx_ConfigBreakDeadTime(&htim1, &sBreakDeadTimeConfig) != HAL_OK) { Error_Handler(); }
    HAL_TIM_MspPostInit(&htim1);
}

static void MX_TIM2_Init(void)
{
    TIM_MasterConfigTypeDef sMasterConfig = {0};
    TIM_OC_InitTypeDef sConfigOC = {0};

    htim2.Instance = TIM2;
    htim2.Init.Prescaler = 83;
    htim2.Init.CounterMode = TIM_COUNTERMODE_UP;
    htim2.Init.Period = 999;
    htim2.Init.ClockDivision = TIM_CLOCKDIVISION_DIV1;
    htim2.Init.AutoReloadPreload = TIM_AUTORELOAD_PRELOAD_DISABLE;
    if (HAL_TIM_PWM_Init(&htim2) != HAL_OK) { Error_Handler(); }
    sMasterConfig.MasterOutputTrigger = TIM_TRGO_RESET;
    sMasterConfig.MasterSlaveMode = TIM_MASTERSLAVEMODE_DISABLE;
    if (HAL_TIMEx_MasterConfigSynchronization(&htim2, &sMasterConfig) != HAL_OK) { Error_Handler(); }
    sConfigOC.OCMode = TIM_OCMODE_PWM1;
    sConfigOC.Pulse = 250;
    sConfigOC.OCPolarity = TIM_OCPOLARITY_HIGH;
    sConfigOC.OCFastMode = TIM_OCFAST_DISABLE;
    if (HAL_TIM_PWM_ConfigChannel(&htim2, &sConfigOC, TIM_CHANNEL_1) != HAL_OK) { Error_Handler(); }
    HAL_TIM_MspPostInit(&htim2);
}

static void MX_ADC1_Init(void)
{
    ADC_ChannelConfTypeDef sConfig = {0};

    hadc1.Instance = ADC1;
    hadc1.Init.ClockPrescaler = ADC_CLOCK_SYNC_PCLK_DIV4;
    hadc1.Init.Resolution = ADC_RESOLUTION_12B;
    hadc1.Init.ScanConvMode = DISABLE;
    hadc1.Init.ContinuousConvMode = DISABLE;
    hadc1.Init.DiscontinuousConvMode = DISABLE;
    hadc1.Init.ExternalTrigConvEdge = ADC_EXTERNALTRIGCONVEDGE_NONE;
    hadc1.Init.ExternalTrigConv = ADC_SOFTWARE_START;
    hadc1.Init.DataAlign = ADC_DATAALIGN_RIGHT;
    hadc1.Init.NbrOfConversion = 1;
    hadc1.Init.DMAContinuousRequests = DISABLE;
    hadc1.Init.EOCSelection = ADC_EOC_SINGLE_CONV;
    if (HAL_ADC_Init(&hadc1) != HAL_OK) { Error_Handler(); }
    sConfig.Channel = ADC_CHANNEL_0;
    sConfig.Rank = 1;
    sConfig.SamplingTime = ADC_SAMPLETIME_3CYCLES;
    if (HAL_ADC_ConfigChannel(&hadc1, &sConfig) != HAL_OK) { Error_Handler(); }
}

static void MX_I2C1_Init(void)
{
    hi2c1.Instance = I2C1;
    hi2c1.Init.ClockSpeed = 100000;
    hi2c1.Init.DutyCycle = I2C_DUTYCYCLE_2;
    hi2c1.Init.OwnAddress1 = 0;
    hi2c1.Init.AddressingMode = I2C_ADDRESSINGMODE_7BIT;
    hi2c1.Init.DualAddressMode = I2C_DUALADDRESS_DISABLE;
    hi2c1.Init.OwnAddress2 = 0;
    hi2c1.Init.GeneralCallMode = I2C_GENERALCALL_DISABLE;
    hi2c1.Init.NoStretchMode = I2C_NOSTRETCH_DISABLE;
    if (HAL_I2C_Init(&hi2c1) != HAL_OK) { Error_Handler(); }
}

static void MX_SPI1_Init(void)
{
    hspi1.Instance = SPI1;
    hspi1.Init.Mode = SPI_MODE_MASTER;
    hspi1.Init.Direction = SPI_DIRECTION_2LINES;
    hspi1.Init.DataSize = SPI_DATASIZE_8BIT;
    hspi1.Init.CLKPolarity = SPI_POLARITY_LOW;
    hspi1.Init.CLKPhase = SPI_PHASE_1EDGE;
    hspi1.Init.NSS = SPI_NSS_SOFT;
    hspi1.Init.BaudRatePrescaler = SPI_BAUDRATEPRESCALER_2;
    hspi1.Init.FirstBit = SPI_FIRSTBIT_MSB;
    hspi1.Init.TIMode = SPI_TIMODE_DISABLE;
    hspi1.Init.CRCCalculation = SPI_CRCCALCULATION_DISABLE;
    hspi1.Init.CRCPolynomial = 10;
    if (HAL_SPI_Init(&hspi1) != HAL_OK) { Error_Handler(); }
}

static void MX_USART2_UART_Init(void)
{
    huart2.Instance = USART2;
    huart2.Init.BaudRate = 115200;
    huart2.Init.WordLength = UART_WORDLENGTH_8B;
    huart2.Init.StopBits = UART_STOPBITS_1;
    huart2.Init.Parity = UART_PARITY_NONE;
    huart2.Init.Mode = UART_MODE_TX_RX;
    huart2.Init.HwFlowCtl = UART_HWCONTROL_NONE;
    huart2.Init.OverSampling = UART_OVERSAMPLING_16;
    if (HAL_UART_Init(&huart2) != HAL_OK) { Error_Handler(); }
}

/* --- MSP de style CubeMX (stm32f4xx_hal_msp.c) ------------------------------------------- */

void HAL_TIM_Base_MspInit(TIM_HandleTypeDef *htim_base)
{
    if (htim_base->Instance == TIM1) {
        __HAL_RCC_TIM1_CLK_ENABLE();
        msp_calls++;
    }
}

void HAL_TIM_PWM_MspInit(TIM_HandleTypeDef *htim_pwm)
{
    if (htim_pwm->Instance == TIM2) {
        __HAL_RCC_TIM2_CLK_ENABLE();
        msp_calls++;
    }
}

void HAL_TIM_MspPostInit(TIM_HandleTypeDef *htim)
{
    GPIO_InitTypeDef GPIO_InitStruct = {0};
    if (htim->Instance == TIM2) {
        __HAL_RCC_GPIOA_CLK_ENABLE();
        /* PA5 ------> TIM2_CH1 */
        GPIO_InitStruct.Pin = GPIO_PIN_5;
        GPIO_InitStruct.Mode = GPIO_MODE_AF_PP;
        GPIO_InitStruct.Pull = GPIO_NOPULL;
        GPIO_InitStruct.Speed = GPIO_SPEED_FREQ_LOW;
        GPIO_InitStruct.Alternate = GPIO_AF1_TIM2;
        HAL_GPIO_Init(GPIOA, &GPIO_InitStruct);
    }
}

void HAL_ADC_MspInit(ADC_HandleTypeDef *hadc)
{
    GPIO_InitTypeDef GPIO_InitStruct = {0};
    if (hadc->Instance == ADC1) {
        __HAL_RCC_ADC1_CLK_ENABLE();
        __HAL_RCC_GPIOA_CLK_ENABLE();
        /* PA0-WKUP ------> ADC1_IN0 */
        GPIO_InitStruct.Pin = GPIO_PIN_0;
        GPIO_InitStruct.Mode = GPIO_MODE_ANALOG;
        GPIO_InitStruct.Pull = GPIO_NOPULL;
        HAL_GPIO_Init(GPIOA, &GPIO_InitStruct);
        msp_calls++;
    }
}

void HAL_I2C_MspInit(I2C_HandleTypeDef *hi2c)
{
    GPIO_InitTypeDef GPIO_InitStruct = {0};
    if (hi2c->Instance == I2C1) {
        __HAL_RCC_GPIOB_CLK_ENABLE();
        /* PB8 ------> I2C1_SCL ; PB9 ------> I2C1_SDA */
        GPIO_InitStruct.Pin = GPIO_PIN_8 | GPIO_PIN_9;
        GPIO_InitStruct.Mode = GPIO_MODE_AF_OD;
        GPIO_InitStruct.Pull = GPIO_NOPULL;
        GPIO_InitStruct.Speed = GPIO_SPEED_FREQ_VERY_HIGH;
        GPIO_InitStruct.Alternate = GPIO_AF4_I2C1;
        HAL_GPIO_Init(GPIOB, &GPIO_InitStruct);
        __HAL_RCC_I2C1_CLK_ENABLE();
        msp_calls++;
    }
}

void HAL_SPI_MspInit(SPI_HandleTypeDef *hspi)
{
    if (hspi->Instance == SPI1) {
        __HAL_RCC_SPI1_CLK_ENABLE();
        msp_calls++;
    }
}

void HAL_UART_MspInit(UART_HandleTypeDef *huart)
{
    if (huart->Instance == USART2) {
        __HAL_RCC_USART2_CLK_ENABLE();
        hdma_usart2_rx.Instance = DMA1_Stream5;
        hdma_usart2_rx.Init.Channel = DMA_CHANNEL_4;
        hdma_usart2_rx.Init.Direction = DMA_PERIPH_TO_MEMORY;
        hdma_usart2_rx.Init.PeriphInc = DMA_PINC_DISABLE;
        hdma_usart2_rx.Init.MemInc = DMA_MINC_ENABLE;
        hdma_usart2_rx.Init.PeriphDataAlignment = DMA_PDATAALIGN_BYTE;
        hdma_usart2_rx.Init.MemDataAlignment = DMA_MDATAALIGN_BYTE;
        hdma_usart2_rx.Init.Mode = DMA_CIRCULAR;
        hdma_usart2_rx.Init.Priority = DMA_PRIORITY_LOW;
        hdma_usart2_rx.Init.FIFOMode = DMA_FIFOMODE_DISABLE;
        if (HAL_DMA_Init(&hdma_usart2_rx) != HAL_OK) { Error_Handler(); }
        __HAL_LINKDMA(huart, hdmarx, hdma_usart2_rx);
        msp_calls++;
    }
}

/* Gestionnaire d'IRQ de style stm32f4xx_it.c */
void DMA1_Stream5_IRQHandler(void)
{
    HAL_DMA_IRQHandler(&hdma_usart2_rx);
}

int main(void)
{
    fake_app_t *fa = fake_app_create(1);
    fake_app_run(fa);

    CHECK(HAL_Init() == HAL_OK);
    MX_DMA_Init();
    MX_TIM1_Init();
    MX_TIM2_Init();
    MX_ADC1_Init();
    MX_I2C1_Init();
    MX_SPI1_Init();
    MX_USART2_UART_Init();
    CHECK(errors == 0);
    CHECK(msp_calls == 6);
    CHECK(huart2.hdmarx == &hdma_usart2_rx && hdma_usart2_rx.Parent == &huart2);

    /* Broches configurées par les MspInit du projet */
    fake_frame_t f;
    CHECK(fake_app_wait(fa, VMCU_MSG_GPIO_CONFIG, 1, 8, 1, 1000, &f)); /* PB8, I2C1 SCL */
    CHECK(f.data[2] == VMCU_GPIO_MODE_ALTERNATE_OD && f.data[5] == 4);
    CHECK(fake_app_wait(fa, VMCU_MSG_GPIO_CONFIG, 0, 0, 1, 1000, &f)); /* PA0, ADC1_IN0 */
    CHECK(f.data[2] == VMCU_GPIO_MODE_ANALOG);
    CHECK(fake_app_wait(fa, VMCU_MSG_GPIO_CONFIG, 0, 5, 1, 1000, &f)); /* PA5, TIM2_CH1 */
    CHECK(f.data[2] == VMCU_GPIO_MODE_ALTERNATE_PP && f.data[5] == 1);

    /* PWM : la valeur de comparaison est mémorisée */
    CHECK(HAL_TIM_PWM_Start(&htim2, TIM_CHANNEL_1) == HAL_OK);
    CHECK(__HAL_TIM_GET_COMPARE(&htim2, TIM_CHANNEL_1) == 250U);
    __HAL_TIM_SET_COMPARE(&htim2, TIM_CHANNEL_1, 500);
    CHECK(__HAL_TIM_GET_COMPARE(&htim2, TIM_CHANNEL_1) == 500U);
    CHECK(__HAL_TIM_GET_AUTORELOAD(&htim2) == 999U);

    /* ADC : conversion immédiate, valeur 0 */
    CHECK(HAL_ADC_Start(&hadc1) == HAL_OK);
    CHECK(HAL_ADC_PollForConversion(&hadc1, 10) == HAL_OK);
    CHECK(HAL_ADC_GetValue(&hadc1) == 0U);

    /* I2C : bus sans composant */
    uint8_t buf[4] = { 1, 2, 3, 4 };
    CHECK(HAL_I2C_Mem_Read(&hi2c1, 0x68 << 1, 0x75, I2C_MEMADD_SIZE_8BIT, buf, 1, 100) == HAL_ERROR);
    CHECK(HAL_I2C_GetError(&hi2c1) == HAL_I2C_ERROR_AF);

    /* SPI : MISO à 0xFF */
    uint8_t tx[3] = { 0x9F, 0, 0 };
    uint8_t rx[3] = { 0 };
    CHECK(HAL_SPI_TransmitReceive(&hspi1, tx, rx, 3, 100) == HAL_OK);
    CHECK(rx[0] == 0xFF && rx[2] == 0xFF);

    /* UART (simulée, test_uart.c) : émission envoyée, réception qui expire,
       émetteur toujours prêt, réception DMA en cours jusqu'à son arrêt */
    const char msg[] = "Bonjour\r\n";
    CHECK(HAL_UART_Transmit(&huart2, (const uint8_t *)msg, (uint16_t)strlen(msg), HAL_MAX_DELAY) == HAL_OK);
    CHECK(HAL_UART_Receive(&huart2, buf, 1, 5) == HAL_TIMEOUT);
    CHECK(__HAL_UART_GET_FLAG(&huart2, UART_FLAG_TXE) == SET);
    CHECK(HAL_UART_Receive_DMA(&huart2, buf, sizeof buf) == HAL_OK);
    CHECK(HAL_UART_GetState(&huart2) == HAL_UART_STATE_BUSY_RX);
    CHECK(HAL_UART_DMAStop(&huart2) == HAL_OK);
    CHECK(HAL_UART_GetState(&huart2) == HAL_UART_STATE_READY);

    /* Chaque fonction non simulée a averti */
    CHECK(fake_app_wait(fa, VMCU_MSG_WARN, -1, -1, 10, 1000, &f));

    vmcu_shutdown();
    fake_app_stop(fa);
    printf("test_stm32_hal_stubs : OK\n");
    return 0;
}
