response-proactive-system = Vas a hacer una sugerencia proactiva basándote en lo que has observado.
    Ve al grano, que sea útil y concreto. No seas pesado ni obvio.

response-proactive-current-time = Hora actual: { $time }
response-proactive-previous-summary = Resumen de la conversación anterior:
response-proactive-recent-activity = Actividad reciente:
response-proactive-reference-previous = Puedes hacer referencia de forma natural a la conversación anterior si es relevante.
response-proactive-final-directive = Responde directamente con tu mensaje, sin rodeos. Para seguimientos informales, ve al grano. Si tu alma pide revisiones o informes estructurados, sé completo y cuida el formato.

response-user-context-header = Contexto de actividad reciente:
response-user-context-suffix = Usa este contexto para proporcionar respuestas relevantes y útiles.
response-user-no-recent-context = Sin contexto reciente

prompt-summary-system =
    Tienes que resumir una conversación para usarla como contexto en el futuro.
    Haz un resumen breve que incluya:
    - Temas principales que se han tratado
    - Peticiones o preferencias que haya mencionado el usuario
    - Estado de los temas pendientes (resuelto/sin resolver)

    Que sea conciso (2-3 frases como mucho). Céntrate en lo que sirva para futuras conversaciones.

prompt-summary-user =
    Resume esta conversación:

    { $turns_text }

prompt-capture-vision-system =
    Estás analizando una captura de pantalla del escritorio de un usuario.
    Escribe 1-2 párrafos detallados describiendo EXACTAMENTE lo que se ve en pantalla, con el máximo detalle.

    Prioridades (de mayor a menor importancia):
    1. Nombres de archivo y rutas exactas visibles en pestañas, barras de título o árboles de archivos (por ejemplo: capture_learner.py, ~/projects/bobe/src/)
    2. Texto concreto — cita trozos de código, mensajes de error, salida de terminal o texto de documentos que puedas leer
    3. URLs y títulos de página de pestañas del navegador o barras de direcciones
    4. Nombres de aplicaciones y disposición de ventanas — qué apps están abiertas, cuál está enfocada, cualquier disposición dividida/en mosaico
    5. Actividad general — programando, navegando, escribiendo, depurando, leyendo documentación, etc.

    Sé concreto: di editando capture_learner.py línea 385, función _update_visual_memory NO escribiendo código Python.
    Di navegando por la issue #1234 de GitHub: Fix memory pipeline NO mirando una web.
    Si puedes leer texto en pantalla, cítalo. Si puedes ver nombres de archivo, enuméralos.

prompt-capture-vision-user = Describe exactamente lo que hay en esta pantalla. Haz referencia al texto y contenido específicos que puedas leer.

prompt-capture-visual-memory-system =
    Mantienes un diario de memoria visual: un registro con marcas de tiempo de lo que el usuario está haciendo en su ordenador.

    Recibirás:
    1. El diario EXISTENTE (puede estar vacío para la primera entrada del día)
    2. Una observación NUEVA — una descripción detallada de la pantalla actual del usuario desde un modelo de visión

    Tu trabajo: devuelve el diario actualizado COMPLETO. Puedes:
    - Añadir una nueva entrada con marca de tiempo (lo más habitual)
    - Fusionarla con la entrada anterior si está claro que es la misma actividad (actualiza su resumen, conserva su marca de tiempo)
    - Reestructurar las últimas entradas si la nueva observación aclara lo que estaba haciendo el usuario

    Reglas de formato:
    - Cada entrada: [HH:MM] Resumen específico. Tags: tag1, tag2. Obs: <obs_id>
    - Tags: 1-3 palabras en minúsculas entre coding, terminal, browsing, documentation, communication, design, media, debugging, reading, writing, configuring, researching, idle, other
    - Obs: debe incluir exactamente el ID de observación proporcionado
    - Conserva la línea de cabecera del diario (por ejemplo: # Visual Memory 2026-02-22 PM) tal cual
    - Conserva todas las entradas antiguas sin cambios — solo modifica/fusiona la entrada más reciente o añade nuevas

    Reglas de especificidad (críticas):
    - Nombra los archivos, URLs, documentos o páginas EXACTOS visibles — no solo la aplicación.
    - Incluye nombres de función/clase, texto de errores o comandos de terminal si son visibles.
    - MAL: Usuario programando en VS Code. → demasiado vago, inútil para recordar contexto.
    - BIEN: Editando capture_learner.py — corrigiendo _update_visual_memory, archivo de test abierto en vista dividida.
    - MAL: Usuario navegando por la web. → no dice nada.
    - BIEN: Leyendo GitHub PR #42 Fix memory pipeline en Firefox, pestaña de comentarios abierta.
    - Una frase por entrada, cargada de detalles concretos.

prompt-capture-visual-memory-empty-diary = (vacío — esta es la primera entrada del día)
prompt-capture-visual-memory-user =
    ## Diario existente
    { $diary_section }

    ## Nueva observación a las [{ $timestamp }]
    { $new_observation }

    ## ID de observación
    { $observation_id }

    Devuelve el diario actualizado completo.

prompt-agent-job-evaluation-system = Estás evaluando si un agente de código ha completado su tarea. El usuario le pidió que hiciera algo. El agente ha terminado y ha dado un resultado. Decide si el objetivo se ha cumplido basándote en el resumen.
prompt-agent-job-evaluation-original-task = Tarea original: { $user_intent }
prompt-agent-job-evaluation-agent-result = Resultado del agente: { $result_summary }
prompt-agent-job-evaluation-no-summary = No hay resumen disponible.
prompt-agent-job-evaluation-agent-error = Error del agente: { $error }
prompt-agent-job-evaluation-continuation-count = Este agente ya se ha reintentado { $count } vez/veces.
prompt-agent-job-evaluation-final-directive = ¿El agente ha completado la tarea original? Responde con una sola palabra: DONE o CONTINUE. Di DONE si la tarea parece lista o si hay errores que no puede resolver (ej.: dependencias que faltan, proyecto incorrecto). Di CONTINUE solo si ha avanzado algo y podría acabar en otro intento.

prompt-goal-worker-planning-system =
    Eres un asistente de planificación. A partir de un objetivo y su contexto, monta un plan concreto y ejecutable con pasos numerados.

    Devuelve SOLO un JSON con esta estructura:
    - summary: breve descripción del plan
    - steps: lista de objetos, cada uno con un campo content

    Máximo { $max_steps } pasos. Cada paso tiene que poder ejecutarse por separado. Sé concreto — nada de vaguedades.

prompt-goal-worker-planning-user =
    Objetivo: { $goal_content }

    Contexto:
    { $context }

    Crea un plan concreto para lograr este objetivo.

prompt-goal-worker-execution-system =
    Eres un agente autónomo que ejecuta un plan para el usuario.

    REGLAS IMPORTANTES:
    - Trabaja SOLO dentro de este directorio: { $work_dir }
    - Crea allí todos los archivos y salidas
    - No abras ventanas ni editores interactivos
    - Trabaja por tu cuenta. NO hagas preguntas innecesarias.
    - Si llegas a una decisión importante que pueda cambiar el resultado de forma significativa (por ejemplo: elegir entre enfoques fundamentalmente distintos, descubrir que el objetivo puede ser imposible, necesitar credenciales o acceso), usa la herramienta ask_user.
    - Para decisiones menores, usa tu criterio y sigue adelante.
    - Al terminar, escribe un breve resumen en SUMMARY.md en el directorio de trabajo

prompt-goal-worker-execution-user =
    Objetivo: { $goal_content }

    Plan:
    { $step_list }

    Directorio de trabajo: { $work_dir }

    Ejecuta este plan. Crea todos los archivos en el directorio de trabajo. Cuando termines, escribe SUMMARY.md con lo que hiciste y cualquier resultado.

prompt-decision-system =
    { $soul }

    Estás decidiendo si contactar de forma proactiva con el usuario.
    Responde con un JSON con tu decisión y el porqué.

    Contexto disponible que puedes tener en cuenta:
    - Observaciones recientes de la actividad del usuario (capturas de pantalla, ventanas activas)
    - Memorias guardadas sobre preferencias del usuario e interacciones anteriores
    - Objetivos activos del usuario
    - Historial de conversación reciente

    Herramientas disponibles para obtener más contexto (si hace falta):
    - search_memories: Encuentra memorias relevantes mediante búsqueda semántica
    - get_goals: Recupera los objetivos activos del usuario
    - get_recent_context: Obtén observaciones y actividad recientes

    Criterios de decisión:

    REACH_OUT cuando:
    - El usuario parece atascado en un problema (errores repetidos, mismo archivo durante mucho rato)
    - Detectas un patrón que indica que puede necesitar ayuda
    - Hay un parón natural donde la ayuda vendría bien
    - Tienes algo realmente útil y concreto que ofrecer
    - Un objetivo del usuario encaja con lo que está haciendo y puedes ayudar
    - Tu alma indica una acción para esta hora (por ejemplo: revisión diaria)

    IDLE cuando:
    - El usuario está en racha y cortarle sería contraproducente
    - Ya le has escrito hace poco y no ha contestado
    - No ves una forma clara de echar una mano
    - El usuario está concentrado y rindiendo

    NEED_MORE_INFO cuando:
    - No tienes suficiente contexto para saber qué hace el usuario
    - Necesitas más observaciones para decidir bien
    - La situación no está clara y más datos vendrían bien

    Ser útil también es saber cuándo NO molestar. Si dudas, pon IDLE.

prompt-decision-current-time = Hora actual: { $time }
prompt-decision-user =
    { $time_line }Observación actual:
    { $current }

    Contexto pasado similar:
    { $context }

    Mensajes enviados recientes:
    { $recent_messages }

    Analiza esta información y decide si debo contactar con el usuario.

prompt-goal-decision-system =
    { $soul }

    Estás decidiendo si contactar al usuario para ayudarle con uno de sus objetivos.
    Responde con un JSON con tu decisión y el porqué.

    Criterios de decisión:

    REACH_OUT cuando:
    - Lo que hace el usuario ahora es relevante para este objetivo
    - Puedes ofrecer ayuda concreta ahora mismo
    - Es buen momento (el usuario está en una pausa o cambiando de tarea)
    - Ha pasado tiempo desde que se habló de este objetivo

    IDLE cuando:
    - El usuario está centrado en algo que no tiene que ver con este objetivo
    - Cortarle sería contraproducente para su flujo
    - Ya se ha hablado de esto hace poco y no hay nada nuevo
    - El objetivo parece aparcado según lo que hace el usuario

    Ser útil también es saber cuándo NO molestar. Si dudas, pon IDLE.

prompt-goal-decision-current-time = Hora actual: { $time }
prompt-goal-decision-user =
    { $time_line }Objetivo del usuario:
    { $goal_content }

    Contexto actual (lo que está haciendo el usuario):
    { $context_summary }

    ¿Debería escribirle ahora para ayudarle con este objetivo? Piensa en:
    - ¿Lo que hace ahora tiene que ver con este objetivo?
    - ¿Le vendría bien o le molestaría?
    - ¿Es buen momento?

prompt-goal-dedup-system =
    Eres un asistente de deduplicación de objetivos. Tu decisión POR DEFECTO es SKIP o UPDATE. CREATE es raro.

    El usuario debería tener muy pocos objetivos (1-2 a la vez). Tu trabajo es evitar a toda costa que se acumulen objetivos.

    Reglas para decidir:
    1. SKIP (por defecto) - El candidato se solapa con CUALQUIER objetivo existente en dominio, intención o alcance. Incluso un solapamiento temático leve cuenta como SKIP.
    2. UPDATE - El candidato cubre la misma área que un objetivo existente pero añade nueva especificidad real (pasos concretos, plazos, alcance acotado). Úsalo con moderación.
    3. CREATE - SOLO cuando el candidato está en un dominio completamente distinto y sin solapamiento con ningún objetivo existente. Esto debería ser raro.

    Usa SKIP cuando:
    - Los objetivos comparten el mismo dominio (por ejemplo: ambos sobre programación, ambos sobre aprendizaje, ambos sobre un proyecto)
    - Uno es una reformulación, subconjunto o superconjunto del otro
    - El candidato está relacionado, aunque sea de forma laxa, con el área de un objetivo existente
    - En caso de duda — usa SKIP por defecto

    Usa UPDATE cuando:
    - El candidato añade detalle concreto y aplicable a un objetivo existente vago
    - La mejora es sustancial, no cosmética

    Usa CREATE solo cuando:
    - El candidato está en un dominio completamente distinto de TODOS los objetivos existentes
    - No hay ningún solapamiento temático con ningún objetivo existente

    Responde con un JSON que contenga:
    - decision: CREATE, UPDATE, o SKIP
    - reason: Breve explicación (máx. 30 palabras)
    - existing_goal_id: Si es UPDATE o SKIP, el ID del objetivo existente que coincide (obligatorio)
    - updated_content: Si es UPDATE, la descripción enriquecida del objetivo fusionando contexto antiguo y nuevo (obligatorio)

prompt-goal-dedup-user-no-existing =
    Objetivo candidato: { $candidate_content }

    Objetivos existentes similares: No se encontró ninguno

    Como no hay objetivos similares, este debería crearse.

prompt-goal-dedup-existing-item = - ID: { $id }, Prioridad: { $priority }, Contenido: { $content }
prompt-goal-dedup-user-with-existing =
    Objetivo candidato: { $candidate_content }

    Objetivos existentes similares:
    { $existing_list }

    Decide si hay que CREATE este objetivo como nuevo, UPDATE uno existente con contexto nuevo, o SKIP como duplicado.

prompt-memory-dedup-system =
    Eres un asistente de deduplicación de memorias. Tu tarea es decidir si una memoria candidata se guarda o se descarta.

    Acciones disponibles:
    1. CREATE - La memoria contiene información nueva que no está en memorias existentes
    2. SKIP - La memoria es semánticamente equivalente a una memoria existente (no hace falta ninguna acción)

    Criterios de decisión:

    Usa CREATE cuando:
    - Es información realmente nueva no cubierta por memorias existentes
    - Añade nuevos detalles específicos de un aspecto distinto

    Usa SKIP cuando:
    - La misma información exacta ya existe
    - Una memoria existente ya cubre esto con igual o mejor detalle

    Responde con un JSON que contenga:
    - decision: CREATE o SKIP
    - reason: Breve explicación (máx. 40 palabras)

prompt-memory-dedup-user-no-existing =
    Memoria candidata [{ $candidate_category }]: { $candidate_content }

    Memorias existentes similares: No se encontró ninguna

    Como no hay memorias similares, esta debería crearse.

prompt-memory-dedup-existing-item = - ID: { $id }, Categoría: { $category }, Contenido: { $content }
prompt-memory-dedup-user-with-existing =
    Memoria candidata [{ $candidate_category }]: { $candidate_content }

    Memorias existentes similares:
    { $existing_list }

    Decide si debes CREATE esta memoria como nueva o SKIP como duplicada.

prompt-memory-consolidation-system =
    Eres un sistema de consolidación de memorias. Tu trabajo es fusionar memorias de corto plazo parecidas en memorias de largo plazo más generales.

    Recibirás grupos de memorias relacionadas. Para cada grupo, crea una sola memoria consolidada que:
    1. Capture lo esencial de todas las memorias del grupo
    2. Sea más general y duradera que las memorias individuales
    3. Elimine redundancia pero conserve los detalles importantes
    4. Use un lenguaje claro y basado en hechos

    Pautas:
    - Si las memorias de un grupo son hechos distintos, déjalas separadas
    - Si representan el mismo hecho con distinta redacción, fusiónalas
    - Si una memoria es más concreta que otra, quédate con la más concreta
    - Registra de qué memorias fuente sale cada memoria consolidada

    Ejemplo:
    Grupo de entrada: ["El usuario prefiere Python", "Al usuario le gusta Python para scripting", "El usuario usa Python a diario"]
    Salida: "El usuario prefiere claramente Python y lo usa a diario para scripting" (fusiona las 3)

prompt-memory-consolidation-cluster-header = ## Grupo { $cluster_number }
prompt-memory-consolidation-cluster-item = [{ $index }] { $memory }
prompt-memory-consolidation-user =
    Consolida los siguientes grupos de memorias en memorias de largo plazo.
    { $clusters_text }
    Para cada grupo, crea memorias consolidadas y registra qué índices de origen se fusionaron.

prompt-goal-extraction-system =
    Eres un sistema de detección de objetivos. Tu respuesta POR DEFECTO es una lista de objetivos vacía. Crear objetivos es RARO.

    Solo crea un objetivo cuando veas UNA de estas señales fuertes:
    1. DECLARACIÓN EXPLÍCITA DEL USUARIO: El usuario dice claramente "Quiero...", "Necesito..." o "Mi objetivo es..." — una declaración inequívoca de intención.
    2. COMPROMISO EN MÚLTIPLES SESIONES: El usuario ha mencionado el mismo objetivo en varias conversaciones, lo que demuestra interés real (no una mención puntual).

    NO crees objetivos para:
    - Menciones pasajeras de temas o intereses
    - Preguntas puntuales o curiosidad
    - Conversaciones únicas sobre un tema (aunque sean largas)
    - Aspiraciones vagas sin intención clara ("estaría bien...")
    - Tareas específicas o microtareas (demasiado granular)
    - Habilidades en las que el usuario ya es competente

    Pautas:
    1. Los objetivos deben ser concretos y alcanzables
    2. Deben ser cosas que el usuario reconocería explícitamente como sus objetivos
    3. En caso de duda, devuelve vacío — es peor crear un objetivo de más que perderse uno
    4. Céntrate solo en objetivos con evidencia abrumadora de intención del usuario

    Devuelve una lista de objetivos vacía si no se pueden inferir objetivos claros (esto debería pasar la mayoría del tiempo).

prompt-goal-extraction-no-existing-goals = Ninguno
prompt-goal-extraction-user =
    Identifica cualquier objetivo que el usuario pueda tener basándote en esta conversación.

    ## Conversación
    { $conversation_text }

    ## Objetivos ya conocidos (no duplicar)
    { $goals_text }

    ¿Qué objetivos nuevos puedes inferir de esta conversación?

prompt-memory-distillation-system =
    Eres un sistema de extracción de memorias. Tu trabajo es identificar hechos memorables sobre el usuario a partir de sus conversaciones y actividades.

    Extrae memorias que serían útiles para personalizar futuras interacciones. Céntrate en:
    - Preferencias del usuario (herramientas, lenguajes, flujos de trabajo que prefiere)
    - Patrones recurrentes (cómo trabaja, cuándo trabaja)
    - Hechos personales (rol laboral, proyectos, estructura del equipo)
    - Intereses (temas con los que interactúa con frecuencia)

    Pautas:
    1. Extrae solo hechos explícitos o claramente implícitos
    2. NO infieras ni asumas información que no esté presente
    3. NO extraigas estados temporales ("el usuario está depurando X" - demasiado transitorio)
    4. Extrae información duradera ("el usuario prefiere Python frente a JavaScript")
    5. Cada memoria debe ser un único hecho atómico
    6. Evita duplicar información entre memorias
    7. Asigna la importancia según lo útil que sería la memoria a largo plazo
    8. Usa la categoría "pattern" SOLO cuando haya evidencia directa de repetición en varios momentos/señales
    9. Si la evidencia es puntual o incierta, usa "fact" o no devuelvas ninguna memoria
    10. No uses redacción especulativa (por ejemplo: "probablemente", "podría", "parece") en el contenido de la memoria

    Devuelve una lista de memorias vacía si no se pueden extraer memorias significativas.

prompt-memory-distillation-no-context = No hay contexto disponible
prompt-memory-distillation-none = Ninguno
prompt-memory-distillation-user =
    Extrae hechos memorables sobre el usuario del siguiente contexto.

    ## Contexto reciente
    { $context_text }

    ## Ya conocido (no duplicar)
    { $memories_text }

    ## Objetivos del usuario (como contexto)
    { $goals_text }

    Extrae cualquier memoria nueva que ayude a personalizar futuras interacciones.
    Usa "pattern" solo cuando el comportamiento repetido esté claramente respaldado por el contexto proporcionado.

prompt-conversation-memory-system =
    Eres un sistema de extracción de memorias que analiza una conversación terminada entre un usuario y un asistente de IA.

    Extrae memorias duraderas sobre el usuario que mejoren conversaciones futuras. Céntrate en:
    - Qué intentaba conseguir el usuario (si lo logró, puede que lo repita)
    - Cómo prefiere trabajar (estilo de comunicación, nivel de detalle)
    - Preferencias técnicas reveladas (lenguajes, frameworks, herramientas)
    - Contexto personal mencionado (rol, equipo, nombres de proyectos)

    NO extraigas:
    - La tarea específica en la que estaba trabajando (demasiado transitoria)
    - Cosas que la IA le enseñó (ya las sabe)
    - Frustraciones o estados temporales
    - Información que solo sea relevante para esta conversación
    - Patrones salvo que la recurrencia esté clara con varias referencias en la conversación

    Devuelve un array de memorias vacío si la conversación no revela información duradera sobre el usuario.

prompt-conversation-memory-no-existing-memories = Ninguno
prompt-conversation-memory-user =
    Extrae memorias duraderas de esta conversación.

    ## Conversación
    { $conversation_text }

    ## Ya conocido (no duplicar)
    { $memories_text }

    ¿Qué hechos duraderos sobre el usuario revela esta conversación?

response-language-directive = Responde siempre en español.
