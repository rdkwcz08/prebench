# Projet de test STM32CubeMX — Nucleo-F401RE, configuration par défaut

Ce dossier sert au critère de passage de la phase 1 (cahier des charges, section 14). Il contient un projet STM32CubeMX **non modifié**, que la cible « sim » doit compiler tel quel.

## Contenu

Projet **généré par STM32CubeMX 6.18.1** (paquet STM32Cube FW_F4 V1.28.3, toolchain **CMake**), sans aucune modification : NUCLEO-F401RE, périphériques initialisés dans leur mode par défaut (LD2 sur PA5, B1 sur PC13 en interruption, USART2 à 115 200 bauds, horloge à 84 MHz).

Traitement par la cible « sim » (`libvmcu/cmake/PrebenchFirmware.cmake`) :

| Fichier | Origine | Cible « sim » |
| --- | --- | --- |
| `cubemx-nucleo-f401re.ioc`, `.mxproject` | Configuration CubeMX | Non utilisés |
| `CMakeLists.txt`, `CMakePresets.json`, `cmake/` | Chaîne CMake de CubeMX (cible ARM) | Lu pour détecter un projet CubeMX brut, puis non utilisé : le projet est compilé via `libvmcu/sim` |
| `Core/Src/main.c`, `stm32f4xx_it.c`, `stm32f4xx_hal_msp.c` | Code généré | Compilés tels quels |
| `Core/Src/system_stm32f4xx.c`, `syscalls.c`, `sysmem.c` | Code généré propre à la puce | Exclus |
| `startup_stm32f401xe.s`, `STM32F401xx_FLASH.ld` | Démarrage et édition de liens ARM | Ignorés |
| `Core/Inc/*.h` | En-têtes générés | Ajoutés avant ceux du shim |
| `Drivers/` | HAL et CMSIS copiés par CubeMX | **Jamais versionné** (`.gitignore`) : lourd, régénérable, remplacé par le shim |

Les fins de ligne CRLF éventuelles sont converties en LF par Git ; le contenu est inchangé.

## Tests

- `test_cubemx` (libvmcu, ctest) : sources compilées telles quelles, initialisation complète sans `Error_Handler`, avertissement « HAL_UART_Init non simulé ». La configuration CMake échoue si `system_stm32f4xx.c`, `syscalls.c` ou `sysmem.c` manquent ou ne sont pas exclus.
- `cubemx_generated_project` (cœur Rust, `cargo test`) : chemin de l'app. Le `CMakeLists.txt` généré est reconnu, le projet est configuré via `libvmcu/sim`, seuls les trois fichiers de `Core/Src` attendus sont compilés, et la simulation démarre jusqu'à USART2.

## Régénérer

1. Installer le paquet **STM32Cube FW_F4 V1.28.3** dans STM32CubeMX (**Help → Manage embedded software packages**).
2. Ouvrir `cubemx-nucleo-f401re.ioc` et cliquer **Generate Code**, sans rien modifier (Project Manager est réglé sur CMake).
3. Depuis la racine du dépôt :

   ```sh
   cmake -S libvmcu -B build-libvmcu -DVMCU_WERROR=ON
   cmake --build build-libvmcu --parallel
   ctest --test-dir build-libvmcu --output-on-failure -R test_cubemx
   ```

## Comportement attendu en phase 1

- **Simulés** : LD2, B1, l'interruption EXTI13 et le temps.
- **USART2 : non simulé.** `HAL_UART_Init` appelle `HAL_UART_MspInit` du projet, ce qui configure PA2 et PA3 en AF7, puis avertit « HAL_UART_Init non simulé ». Le moniteur série arrive en phase 2.
- **Boucle principale.** La boucle `while (1) {}` de CubeMX n'appelle jamais libvmcu : elle occupe un cœur à 100 %. Les micro-pauses de libvmcu ne peuvent pas agir sans appel à libvmcu. Le plafond CPU par processus (section 11.2) arrive en phase 2.
