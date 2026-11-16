# Projet de référence STM32CubeMX — Nucleo-F401RE, configuration par défaut + I2C1

Ce dossier sert au critère de passage de la phase 1 (cahier des charges, section 14) et aux tests de l'I2C (phase 3). Il contient un projet STM32CubeMX que la cible « sim » doit compiler tel quel : le code généré n'est jamais modifié, seul du code est ajouté dans les sections `USER CODE` de `Core/Src/main.c`, que CubeMX conserve à chaque régénération.

## Contenu

Projet **généré par STM32CubeMX 6.18.1** (paquet STM32Cube FW_F4 V1.28.3, toolchain **CMake**) : NUCLEO-F401RE, périphériques initialisés dans leur mode par défaut (LD2 sur PA5, B1 sur PC13 en interruption, USART2 à 115 200 bauds, horloge à 84 MHz), plus **I2C1** sur PB8 (SCL) et PB9 (SDA), 100 kHz, interruption I2C1 event activée (ajouté le 15 novembre 2026, sans autre changement).

Code ajouté dans les sections `USER CODE` de `main.c` : lecture du registre 0x00 du composant en 0x50 par `HAL_I2C_Mem_Read_IT` ; `HAL_I2C_MemRxCpltCallback` lève un drapeau, la boucle principale affiche « I2C 0x50 reg 0x00 = 0x2A » sur USART2, puis attend 10 ms (`HAL_Delay`) à chaque tour. `prebench.toml` place un joker I2C en 0x50 (registre 0x00 = 0x2A) sur PB8 et PB9.

Traitement par la cible « sim » (`libvmcu/cmake/PrebenchFirmware.cmake`) :

| Fichier | Origine | Cible « sim » |
| --- | --- | --- |
| `cubemx-nucleo-f401re.ioc`, `.mxproject` | Configuration CubeMX | Non utilisés |
| `CMakeLists.txt`, `CMakePresets.json`, `cmake/` | Chaîne CMake de CubeMX (cible ARM) | Lu pour détecter un projet CubeMX brut, puis non utilisé : le projet est compilé via `libvmcu/sim` |
| `Core/Src/main.c`, `stm32f4xx_it.c`, `stm32f4xx_hal_msp.c` | Code généré (+ `USER CODE` de `main.c`) | Compilés tels quels |
| `Core/Src/system_stm32f4xx.c`, `syscalls.c`, `sysmem.c` | Code généré propre à la puce | Exclus |
| `startup_stm32f401xe.s`, `STM32F401xx_FLASH.ld` | Démarrage et édition de liens ARM | Ignorés |
| `Core/Inc/*.h` | En-têtes générés | Ajoutés avant ceux du shim |
| `Drivers/` | HAL et CMSIS copiés par CubeMX | **Jamais versionné** (`.gitignore`) : lourd, régénérable, remplacé par le shim |
| `prebench.toml` | Paillasse Prebench (joker I2C) | Lu par l'app |

Les fins de ligne CRLF écrites par CubeMX sont converties en LF par Git ; le contenu est inchangé.

## Tests

- `test_cubemx` (libvmcu, ctest) : sources compilées telles quelles face à une fausse app qui simule un esclave en 0x50 ; initialisation complète sans `Error_Handler`, USART2 et I2C1 simulés (PB8 et PB9 en AF4 drain ouvert, 100 kHz), ligne « I2C 0x50 reg 0x00 = 0x2A » sur USART2. La configuration CMake échoue si `system_stm32f4xx.c`, `syscalls.c` ou `sysmem.c` manquent ou ne sont pas exclus.
- `cubemx_generated_project` (cœur Rust, `cargo test`) : chemin de l'app. Le `CMakeLists.txt` généré est reconnu, le projet est configuré via `libvmcu/sim`, seuls les trois fichiers de `Core/Src` attendus sont compilés, la simulation démarre, USART2 est simulée et la lecture du joker de `prebench.toml` s'affiche.

## Régénérer

1. Installer le paquet **STM32Cube FW_F4 V1.28.3** dans STM32CubeMX (**Help → Manage embedded software packages**).
2. Ouvrir `cubemx-nucleo-f401re.ioc` et cliquer **Generate Code** (Project Manager est réglé sur CMake). Les sections `USER CODE` de `main.c` sont conservées.
3. Vérifier avec `git diff` que seul ce qui a été changé dans CubeMX a bougé, puis, depuis la racine du dépôt :

   ```sh
   cmake -S libvmcu -B build-libvmcu -DVMCU_WERROR=ON
   cmake --build build-libvmcu --parallel
   ctest --test-dir build-libvmcu --output-on-failure -R test_cubemx
   ```

## Comportement attendu

- **Simulés** : LD2, B1, l'interruption EXTI13, le temps, USART2 (phase 2) et I2C1 (phase 3).
- **Interruption I2C1.** `HAL_I2C_Mem_Read_IT` fait la transaction aussitôt ; `HAL_I2C_MemRxCpltCallback` est appelé par le fil des interruptions de libvmcu, sans passer par `I2C1_EV_IRQHandler`.
- **Boucle principale.** Le code ajouté attend 10 ms par tour : la boucle n'occupe plus un cœur à 100 % (la boucle vide générée par CubeMX n'appelait jamais libvmcu).
