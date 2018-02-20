# Dark Binding

This application runs in the background and automatically swaps your League of Legends keybindings to the champion you're about to play.

## Installation

Prebuilt binaries coming soon.

## How it works

The application listens for when you lock in champion select, it then backs up your League config and clones it into a separate directory whilst also making a hard link back to the League config directory. Once the game ends it updates your original config file with any changes not made to your keybindings and restores it. When you next pick that champion, the champion specific changes you made will remain.

## Roadmap

~~1. Allow users to define groups of champions instead of making a virtual config for every champion~~
