# Modèle : blinky STM32 HAL (Nucleo-F401RE)

LD2 clignote toutes les 500 ms. Chaque appui sur le bouton bleu B1 fait passer la période de 500 ms à 100 ms, puis de 100 ms à 500 ms. Le code est du HAL STM32 standard, organisé comme un projet STM32CubeMX (`Core/Inc`, `Core/Src`).

## Cible « sim »

Depuis la racine du dépôt Prebench :

```sh
cmake -S templates/blinky-stm32-hal -B build-blinky
cmake --build build-blinky
```

Sous Windows, ajouter `-G "MinGW Makefiles"` à la première commande.

L'exécutable `build-blinky/firmware` est lancé par Prebench. Lancé seul, il s'arrête avec le message « PREBENCH_SOCKET absent ».

## Vraie carte

Ce modèle ne contient que les sources de l'application. Pour flasher la Nucleo, il faut un projet STM32CubeMX ou STM32CubeIDE pour la NUCLEO-F401RE ; y copier `Core/Src/main.c`, `Core/Inc/main.h` et `Core/Src/stm32f4xx_it.c`. Le code est le même pour la carte et pour la simulation.
