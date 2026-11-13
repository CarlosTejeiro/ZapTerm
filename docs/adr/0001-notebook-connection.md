# ADR 0001 — Conexión de notebooks (Jupyter / VS Code) a ZapTerm

- **Estado:** Aceptada (2026-10-23) — implementada en 0.26.0
- **Fecha:** 2026-10-23
- **Contexto previo:** Python scripting (0.25.0), `docs/user-guide/scripting.md`

## 1. Problema

Hoy un script de ZapTerm se escribe entero y se ejecuta de principio a fin
(Tools → Run Python script…). Para escribirlo bien hace falta saber qué
devuelve cada comando en un equipo real, y el dry run no lo dice: `run()`
devuelve una cadena vacía. El ciclo es "editar → ejecutar todo → mirar el
panel → repetir".

Queremos lo que da un notebook: ejecutar un trozo, ver la salida real, escribir
el siguiente trozo sobre esa salida, con las variables y las pestañas abiertas
vivas entre celdas.

## 2. Propuesta

No construir un notebook dentro de ZapTerm. Permitir que un **notebook externo**
(VS Code, JupyterLab, o una consola `python`) se conecte a un ZapTerm abierto y
use el mismo módulo `zapterm` que los scripts.

```python
import zapterm
zapterm.connect()                       # se engancha al ZapTerm abierto

fw = zapterm.open("FGT-60F-01")        # abre la sesión guardada en una pestaña
out = fw.run("get vpn ipsec tunnel summary")
print(out)                           # salida real, en la celda
```

Siguiente celda, sobre esa misma salida:

```python
import re
re.findall(r"'(\S+)'.*?selectors\(total,up\): (\d+)/(\d+)", out)
```

ZapTerm sigue haciendo el trabajo en sus pestañas (lo ves en directo); el notebook
solo decide, igual que un script.

### Por qué un notebook externo y no uno propio

| | Notebook propio en ZapTerm | Notebook externo (propuesta) |
|---|---|---|
| Editor (autocompletado, resaltado, deshacer…) | Hay que construirlo | El de VS Code / Jupyter |
| Celdas, salida, variables entre celdas | Hay que construirlo (kernel propio) | Ya existe (Jupyter) |
| Guardar / compartir `.ipynb` | Hay que construirlo | Ya existe |
| Coste en ZapTerm | Alto (UI grande + gestión de kernel) | Bajo (ver §5) |
| Requisitos en el equipo | Solo Python | Python + `ipykernel` (+ VS Code o JupyterLab) |

El único coste para el usuario es instalar `ipykernel` en el Python que use el
notebook; VS Code ofrece instalarlo solo al abrir un notebook con un entorno que
no lo tiene ([wiki de vscode-jupyter](https://github.com/microsoft/vscode-jupyter/wiki/Jupyter-Kernels-and-the-Jupyter-Extension/d30ad4d26109128a5db5aa297aa0bda87567c37c)).

## 3. Cómo funciona

### 3.1 Activar la conexión en ZapTerm

**Tools → Allow notebook connection** (desactivado por defecto). Al activarlo,
ZapTerm:

1. abre un puerto local (`127.0.0.1`, puerto aleatorio) con un **token nuevo**;
2. escribe `<data dir>/scripting/connection.json` (`{"port": …, "token": …}`),
   legible solo por tu usuario (permisos `0600` en Linux/macOS);
3. muestra el panel de scripts en modo "Notebook: esperando conexión".

Al desactivarlo, o al cerrar ZapTerm, se cierra el puerto, se borra el fichero y
el token deja de valer. Cada activación genera un token distinto.

### 3.2 Conectar desde el notebook

`zapterm.connect()` lee `connection.json` (ZapTerm sabe dónde está su carpeta de
datos; el módulo la busca en las rutas por defecto de cada sistema, o se le
pasa `zapterm.connect(path=…)`). Alternativa sin fichero: el panel muestra una
línea para copiar, `zapterm.connect(port=…, token=…)`.

Para que `import zapterm` funcione en el notebook: `pip install` no hace falta;
basta con que `zapterm.py` esté junto al `.ipynb` (la carpeta de ejemplos ya tiene
una copia) o en el `sys.path`. El panel muestra la línea `sys.path.insert(…)`
exacta.

### 3.3 Mientras está conectado

- Un solo notebook conectado a la vez. Un segundo `connect()` es rechazado
  con un mensaje claro.
- Mismas llamadas que en los scripts, más dos pensadas para notebooks:
  - `zapterm.tabs()` — pestañas abiertas ahora (nombre, plataforma);
  - `zapterm.tab("FGT-60F-01")` — engancharse a una pestaña **ya abierta** por
    su nombre, sin abrir otra.
- `zapterm.session()` en un notebook devuelve la pestaña con el foco **en ese
  momento** (en un script es la que tenía el foco al arrancar).
- El panel de ZapTerm muestra "Notebook conectado", cada acción (como hoy con los
  scripts) y un botón **Disconnect**.
- `confirm()` pregunta en ZapTerm, igual que en los scripts.

### 3.4 Interrumpir una celda

Si paras una celda (botón ■ en VS Code) mientras ZapTerm espera un prompt, el
módulo avisa a ZapTerm para que cancele esa espera, y la siguiente celda empieza
limpia. Hoy el módulo no lo necesita (un script parado muere entero), pero en
un notebook el kernel sigue vivo: sin esto, la respuesta tardía de la celda
cancelada se tomaría como la respuesta de la siguiente. Se resuelve
emparejando cada respuesta con su `id` de petición y descartando las
huérfanas, más una petición `cancel`.

Si el kernel se reinicia o se cierra VS Code, ZapTerm detecta la conexión cerrada
y vuelve a "esperando conexión" (sin tocar las pestañas).

## 4. Seguridad

- Solo escucha en `127.0.0.1`: nada desde la red.
- Desactivado por defecto; se activa a mano y se apaga al cerrar ZapTerm.
- Token aleatorio por activación; sin token, la conexión se cierra (como hoy).
- `connection.json` solo legible por tu usuario. Un programa que ya corre con
  tu usuario podría leerlo — el mismo nivel de confianza que hoy tiene sobre
  tus ficheros. Por eso el interruptor y el panel visible: siempre sabes si
  hay algo conectado y lo puedes cortar.
- Las contraseñas siguen sin llegar a Python: `send_secret()` escribe desde el
  vault directamente en la pestaña.
- Opcional: desconexión automática tras N minutos sin actividad.

## 5. Coste estimado

| Parte | Trabajo |
|---|---|
| Rust | Listener persistente (en vez de uno por ejecución), `connection.json`, comandos enable/disable |
| Frontend | Opción en Tools, estados del panel (esperando / conectado / desconectar), `tabs` / `tab` |
| Módulo `zapterm` | `connect()`, ids emparejados + `cancel`, `tabs()`, `tab()` |
| Ejemplo | `examples/explore.ipynb` (abrir equipo, ver túneles, parsear) |
| Tests | Rust (conectar, token, un cliente, desconexión, cancel) + E2E con un kernel real |
| Docs | Guía nueva (ver §7) |

Una PR, del tamaño de la de scripting o algo menor: reutiliza todo el canal
actual (JSON lines, despacho en el frontend, panel).

## 6. Fuera de alcance (por ahora)

- Notebook dentro de ZapTerm.
- FortiGate simulado.
- Modo solo lectura (`get`/`show` permitidos, el resto bloqueado): encaja muy
  bien con notebooks y se puede hacer después, por separado.
- Conexión desde otra máquina (solo local, a propósito).

## 7. Documentación (cuando esté aprobado y cerrado)

Una guía para alguien que nunca ha usado ZapTerm, en `docs/user-guide/`, con
capturas:

1. Primeros pasos con ZapTerm (sesiones, carpetas, vault) — lo justo para seguir
   la guía.
2. Scripts: qué son, qué hacen y cómo; el ejemplo `sase_reset.py` paso a paso;
   dry run.
3. Notebooks: instalar `ipykernel`, activar la conexión, primera celda,
   explorar y pasar lo que funciona a un script.

Capturas: las de ZapTerm se sacan de la app real. Las del notebook, de
JupyterLab (mismo `.ipynb` que en VS Code); en este entorno no hay VS Code
para capturarlo.

## 8. Decisiones

1. Enfoque aprobado (notebook externo + `zapterm.connect()`).
2. Desconexión automática: sí, 60 min sin llamadas del notebook
   (`notebook.idle_minutes`, `0` = nunca).
3. Idioma de la guía para principiantes: pendiente.
