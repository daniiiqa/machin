# machín

La pantalla de tu otra compu como monitor extra, en cualquier dirección: la
laptop como segundo monitor de la PC, o el monitor de la PC como segundo de la
laptop. Para discada (Hyprland 0.57+, config en Lua).

La principal crea un monitor virtual (`MACHIN`, con `hyprctl output create
headless`) junto al suyo y lo transmite con Sunshine. La otra lo muestra a
pantalla completa con Moonlight. Para Hyprland es un monitor de verdad: el
mouse cruza solo, le tocan workspaces y las ventanas van y vienen. Al apagarlo
todo regresa a tu pantalla.

```
machin prende [ANCHOxALTO] [--izquierda]   esta es la principal (de entrada 1920x1200, a la derecha)
machin apaga                               lo quita; las ventanas regresan
machin presta HOST                         esta es la pantalla: Moonlight contra HOST
machin estado                              prendido ANCHOxALTO | apagado
```

Necesita `sunshine-bin` (AUR) y `moonlight-qt` en las dos máquinas, emparejadas
una vez: el panel de Sunshine en https://localhost:47990 y el PIN que da
Moonlight al agregar la otra compu por IP.

machín deja en `~/.config/sunshine/sunshine.conf` `capture = wlr` y
`output_name = MACHIN` (por nombre: con el número, Sunshine transmite el
monitor principal); lo demás se respeta. La regla del monitor se pone en
caliente y una recarga de la config la borra, así que `prende` deja un
vigilante (`machin vigila`) que la vuelve a poner y se va cuando MACHIN se va.

Ajustes en `~/.config/machin/machin.conf` (clave=valor): `tam=1920x1200`,
`lado=derecha|izquierda`, `fps=60`, `bitrate=35000`, `app=Desktop`.

MIT.
