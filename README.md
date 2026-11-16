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
machin                                     el atajo (SUPER+Shift+M en discada): busca la otra
                                           compu, le pide su pantalla y la pone junto a la tuya;
                                           si ya está, la quita
machin menu                                sonido, lado, calidad y compus de confianza
machin escucha                             el servicio de cada máquina (al iniciar sesión)
machin prende [ANCHOxALTO] [--izquierda]   a mano: esta es la principal
machin apaga                               lo quita; las ventanas regresan
machin presta HOST                         a mano: esta es la pantalla, Moonlight contra HOST
machin estado                              prendido ANCHOxALTO | apagado
```

Cada máquina corre `machin escucha`: se anuncia en la red (mDNS
`_machin._tcp`, con su nombre y el tamaño de su pantalla) y atiende en el
puerto 47392. Al apretar el atajo, la otra pregunta *Aceptar / Siempre / No*
(con *Siempre* ya no vuelve a preguntar; la lista está en
`~/.config/machin/confianza`). Solo acepta compus emparejadas en su Moonlight,
y Moonlight fija su certificado. Al cerrar Moonlight allá, se apaga aquí.

Necesita `sunshine-bin` (AUR) y `moonlight-qt` en las dos máquinas, emparejadas
una vez: el panel de Sunshine en https://localhost:47990 y el PIN que da
Moonlight al agregar la otra compu por IP.

machín deja en `~/.config/sunshine/sunshine.conf` `capture = wlr` y
`output_name = MACHIN` (por nombre: con el número, Sunshine transmite el
monitor principal); lo demás se respeta. La regla del monitor se pone en
caliente y una recarga de la config la borra, así que `prende` deja un
vigilante (`machin vigila`) que la vuelve a poner y se va cuando MACHIN se va.

Ajustes en `~/.config/machin/machin.conf` (clave=valor), también desde
`machin menu` (en discada: el menú de discada, o clic en el ícono de empalme):
`sonido=aqui|alla|ambos` (dónde suena: en la principal, en la otra o en las
dos; de entrada solo en la principal), `lado=derecha|izquierda`,
`bitrate=35000`, `fps=60`, `tam=1920x1200`, `app=Desktop`. Los decide la
principal y se los manda a la otra; si cambian con machín prendido, se
reconecta solo.

Mientras está activo, machín deja en `$XDG_RUNTIME_DIR/machin` un archivo
`<pid>-principal` o `<pid>-presta` (`<rol> <otra> <tam>`), que lee el módulo
`machin` de empalme.

MIT.
