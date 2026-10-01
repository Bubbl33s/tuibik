# Configuración de protección para `main`

Después de que el workflow **CI** haya terminado al menos una vez en GitHub,
crea un ruleset para la rama `main` en **Settings > Rules > Rulesets**.

## Reglas requeridas

- Requerir una pull request antes de fusionar.
- Requerir que las conversaciones estén resueltas.
- Descartar aprobaciones obsoletas cuando se suban nuevos commits.
- Bloquear force pushes y la eliminación de la rama.
- Requerir estos checks, cuyos nombres deben permanecer estables:
  - `fmt`
  - `clippy`
  - `test`
  - `build`

Mientras Valeria sea la única mantenedora, no configures un número mínimo de
aprobaciones: GitHub no permite que la autora apruebe su propia pull request.
Cuando haya otra mantenedora, configura una aprobación obligatoria y revisión
de code owner según la [política de gobernanza](../docs/governance.md).

## Verificación

Abre una pull request de prueba que haga fallar temporalmente uno de los cuatro
checks. GitHub debe impedir fusionarla. Revierte ese cambio de prueba antes de
fusionar la configuración.
