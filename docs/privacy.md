# Privacy policy: nddev-browseruse

This policy covers the nddev-browseruse Chromium extension and its local host.
The standalone extraction is a release candidate under qualification.
[Русская версия ниже](#политика-конфиденциальности-nddev-browseruse).

## Browser access

The extension connects to a host installed on the same device through native
messaging. Agents you run use that host to control their own tab groups. They
can read page text, DOM, URLs, network activity and console messages; click,
type, navigate, download files, take screenshots and make recordings in those
tabs. Moving a tab into an agent group grants the agent access to that tab.
The extension does not attach its debugger to tabs outside agent groups.

Downloads initiated by agent tabs are copied to their session area. Only after
the copy succeeds does the extension remove that download's browser copy and
history entry. Other downloads are not processed. Explicit recordings can
contain screen content, your actions and audio when requested.

The extension stores preferences locally, including access, observation,
language, private-site rules and group membership. The host stores its local
configuration, session artifacts and any activity journal separately.

## Observation and storage

- **Full** keeps a local activity journal: execution code and results, browser
  actions, navigations, selected network metadata and textual bodies, warnings,
  errors, dialogs and downloads. Recognized credentials and secret fields are
  redacted before records are written. Redaction does not make arbitrary page
  content non-sensitive. Journals have size limits and one-week retention.
- **Standard** keeps event names, counts, durations, outcomes and error classes.
  Operational metadata has no URL, title, code or page-content fields.
- **Off** stops new observation collection and invalidates queued records.
  Re-enabling observation does not replay records from the earlier policy.
  Off does not erase records or artifacts already saved.
- **Private sites**, including their subdomains, are omitted from journal
  content. Minimal activity metadata can remain. This setting does not remove
  the agent's permission to work on those sites or delete earlier records.

Recordings and requested downloads/screenshots are task artifacts. They have
separate explicit lifecycles; they are not ordinary metadata diagnostics.
The local host writes metadata diagnostics to stderr and does not automatically
export them to a vendor service. Removing the extension deletes its preferences,
not the host's configuration, journal or artifacts. Remove those state files
separately when you want to erase them.

## Where data goes

The extension sends browser data to the local host. It has no vendor cloud
relay or built-in AI provider. The agent receives the results it requests.
If your agent uses an online model, the data it sends is covered by that
provider's terms and privacy policy.

Optional remote-device use invokes a CLI through your configured SSH or command
transport. Optional Android use connects through your ADB server. Agents can
retrieve results and artifacts through those owner-configured paths. An NDS
integration can compose its own observation transport and consent; the
standalone product includes no NDS telemetry SDK or automatic central exporter.

## Your controls

The access switch stops agent control and drains sessions. Observation controls
what new diagnostic data is retained; private-site rules restrict journal
content. Stop an explicit recording to end that recording. Before granting a
tab, consider which data the agent and its model provider may receive.

For privacy questions contact [danil@nddev.ai](mailto:danil@nddev.ai).
Report non-sensitive defects in [GitHub issues](https://github.com/NDDev-Technocracy-OpenNetwork/nddev-browseruse/issues)
and vulnerabilities as described in [SECURITY.md](../SECURITY.md). Policy changes
are visible in this file's public history.

# Политика конфиденциальности nddev-browseruse

Политика относится к расширению nddev-browseruse для Chromium и его локальному
хосту. Самостоятельная реализация пока проходит проверку перед выпуском.

## Доступ к браузеру

Расширение связывается через native messaging с хостом на том же устройстве.
Агенты, которых вы запускаете, управляют своими группами вкладок: читают текст,
DOM, адреса, сетевую активность и сообщения консоли, нажимают элементы, вводят
текст, переходят по ссылкам, скачивают файлы, делают снимки и записи.
Перемещая вкладку в группу агента, вы предоставляете ему доступ к ней.
Расширение не подключает отладчик к вкладкам вне групп агентов.

Загрузки из вкладок агента копируются в каталог сессии. Только после успешного
копирования расширение удаляет браузерную копию и запись этой загрузки.
Другие загрузки оно не обрабатывает. Явно запрошенная запись может содержать
изображение страницы, ваши действия и звук, если он был запрошен.

Расширение локально хранит настройки доступа, наблюдения, языка, приватных
сайтов и групп. Хост отдельно хранит конфигурацию, артефакты и журнал активности.

## Наблюдение и хранение

- **Полное** сохраняет локальный журнал: код и результаты выполнения, действия,
  переходы, выбранные сетевые данные и текстовые тела, предупреждения, ошибки,
  диалоги и загрузки. Известные поля с учётными данными и секретами удаляются
  перед записью. Произвольный текст страницы может оставаться чувствительным.
  Журнал ограничен по размеру и хранится одну неделю.
- **Стандарт** сохраняет названия событий, счётчики, длительности, результаты
  и классы ошибок. Метаданные не содержат адресов, заголовков страниц, кода
  или содержимого страниц.
- **Выкл.** прекращает новый сбор и аннулирует очередь записей. Повторное
  включение не выдаёт записи прежнего режима. Уже сохранённые записи и
  артефакты при этом не удаляются.
- **Приватные сайты** вместе с поддоменами не попадают в журнал своим
  содержимым. Минимальные метаданные активности могут сохраняться. Настройка
  не запрещает агенту работать на сайте и не удаляет прежние записи.

Явно запрошенные записи, загрузки и снимки — артефакты задачи со своим сроком
жизни. Они отделены от диагностических метаданных. Хост пишет диагностику
локально в stderr и автоматически не отправляет её сервису разработчика.
Удаление расширения удаляет его настройки, но не конфигурацию, журнал и
артефакты хоста. Для удаления этих данных отдельно удалите файлы состояния.

## Передача данных

Расширение передаёт данные локальному хосту. Встроенного облачного ретранслятора
и провайдера ИИ нет. Агент получает запрошенные результаты. Если он использует
онлайн-модель, к отправленным ей данным применяются правила её провайдера.

Необязательная работа с другим устройством запускает CLI через настроенный вами
SSH или командный транспорт. Android подключается через ваш сервер ADB.
Агенты могут получать результаты и артефакты по этим настроенным каналам.
Интеграция NDS может отдельно подключить свой транспорт наблюдения и согласие
на сбор. В самостоятельном продукте нет SDK телеметрии NDS и автоматического
экспорта в центральную систему.

## Ваши настройки

Переключатель доступа останавливает управление и завершает работу сессий.
Профиль наблюдения определяет сохранение новых диагностических данных,
а приватные сайты ограничивают содержимое журнала. Явную запись завершает
кнопка остановки записи. Предоставляя вкладку агенту, учитывайте, какие данные
могут получить он и провайдер его модели.

По вопросам конфиденциальности пишите на [danil@nddev.ai](mailto:danil@nddev.ai).
Обычные ошибки без приватных данных можно сообщать в
[GitHub issues](https://github.com/NDDev-Technocracy-OpenNetwork/nddev-browseruse/issues),
уязвимости — по [SECURITY.md](../SECURITY.md). Изменения политики видны в истории файла.
