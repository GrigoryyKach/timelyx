//! All interface text, in English, Russian and Ukrainian. One row per string: `key: en, ru, uk;`.
//! Style: sentence case, no emoji, no trailing periods on labels, no exclamation marks.

use crate::i18n::Table;

macro_rules! strings {
    (
        text { $( $name:ident : $en:expr, $ru:expr, $uk:expr; )* }
    ) => {
        #[allow(dead_code)]
        pub struct S {
            $( pub $name: &'static str, )*
        }
        const EN: S = S { $( $name: $en, )* };
        const RU: S = S { $( $name: $ru, )* };
        const UK: S = S { $( $name: $uk, )* };
        pub static TABLE: Table<S> = Table { en: &EN, ru: &RU, uk: &UK };
    };
}

strings! {
    text {
        // Navigation
        nav_today: "Today", "Сегодня", "Сьогодні";
        nav_trends: "Week and month", "Неделя и месяц", "Тиждень і місяць";
        nav_apps: "Apps and sites", "Программы и сайты", "Програми та сайти";
        nav_categories: "Categories", "Категории", "Категорії";
        nav_limits: "Limits", "Лимиты", "Ліміти";
        nav_settings: "Settings", "Настройки", "Налаштування";
        collapse_sidebar: "Collapse sidebar", "Свернуть панель", "Згорнути панель";
        expand_sidebar: "Expand sidebar", "Развернуть панель", "Розгорнути панель";

        // Today
        active_time: "Active time", "Активное время", "Активний час";
        day_timeline: "Day timeline", "Лента дня", "Стрічка дня";
        by_category: "By category", "По категориям", "За категоріями";
        top_items: "Top apps and sites", "Чаще всего", "Найчастіше";
        now_in: "Now", "Сейчас", "Зараз";
        now_paused: "Tracking is paused", "Учёт приостановлен", "Облік призупинено";
        now_paused_until: "Paused until {time}", "Пауза до {time}", "Пауза до {time}";
        now_idle: "Away from the keyboard", "Вы отошли", "Ви відійшли";
        now_locked: "Computer is locked or asleep", "Компьютер заблокирован или спит", "Комп'ютер заблоковано або він спить";
        now_nothing: "No window in focus", "Нет активного окна", "Немає активного вікна";
        nothing_today: "Nothing tracked yet today", "Сегодня пока ничего не записано", "Сьогодні ще нічого не записано";
        nothing_today_hint: "Timelyx records the programs and sites you use while it runs in the background", "Timelyx записывает программы и сайты, которыми вы пользуетесь, пока работает в фоне", "Timelyx записує програми та сайти, якими ви користуєтеся, поки працює у фоні";
        tracker_off: "The tracker is not running", "Учёт не запущен", "Облік не запущено";
        tracker_off_hint: "Nothing is recorded while it is off", "Пока он выключен, ничего не записывается", "Поки він вимкнений, нічого не записується";
        start_tracking: "Start tracking", "Запустить учёт", "Запустити облік";
        wayland_title: "Tracking isn't available on Wayland", "Учёт недоступен в Wayland", "Облік недоступний у Wayland";
        wayland_body: "Wayland does not let programs see which window is active. Log in with an X11 session to track your time. Reports and settings work as usual.", "Wayland не позволяет программам узнать, какое окно активно. Войдите в сеанс X11, чтобы учитывать время. Отчёты и настройки работают как обычно.", "Wayland не дозволяє програмам дізнатися, яке вікно активне. Увійдіть у сеанс X11, щоб вести облік часу. Звіти й налаштування працюють як завжди.";
        tracker_error: "The tracker could not start", "Не удалось запустить учёт", "Не вдалося запустити облік";
        sessions_n: "{n} sessions", "сессий: {n}", "сесій: {n}";

        // Trends
        period_week: "Week", "Неделя", "Тиждень";
        period_month: "Month", "Месяц", "Місяць";
        total: "Total", "Всего", "Усього";
        avg_per_day: "Average per day", "В среднем за день", "У середньому за день";
        vs_prev_week: "vs previous week", "к прошлой неделе", "до минулого тижня";
        vs_prev_month: "vs previous month", "к прошлому месяцу", "до минулого місяця";
        no_comparison: "No data before", "Раньше данных нет", "Раніше даних немає";
        by_day: "By day", "По дням", "За днями";
        heatmap_title: "When you are active", "Когда вы активны", "Коли ви активні";
        heatmap_hint: "Hours of the day by weekday, darker is more time", "Часы суток по дням недели, темнее значит больше времени", "Години доби за днями тижня, темніше означає більше часу";
        longest_focus: "Longest focus", "Самый долгий фокус", "Найдовший фокус";
        longest_focus_hint: "Continuous time in one category without a switch", "Непрерывное время в одной категории без переключений", "Безперервний час в одній категорії без перемикань";
        focus_none: "None yet", "Пока нет", "Поки немає";
        busiest_day: "Busiest day", "Самый насыщенный день", "Найнасиченіший день";
        site_inherit: "Same as browser", "Как у браузера", "Як у браузера";
        today_label: "Today", "Сегодня", "Сьогодні";
        yesterday_label: "Yesterday", "Вчера", "Вчора";
        prev: "Previous", "Назад", "Назад";
        next: "Next", "Вперёд", "Вперед";
        this_period: "Current", "Текущая", "Поточна";
        less: "Less", "Меньше", "Менше";
        more: "More", "Больше", "Більше";
        hours_active: "of activity", "активности", "активності";

        // Apps and sites
        apps_title: "Apps and sites", "Программы и сайты", "Програми та сайти";
        range_today: "Today", "Сегодня", "Сьогодні";
        range_7: "7 days", "7 дней", "7 днів";
        range_30: "30 days", "30 дней", "30 днів";
        range_all: "All time", "Всё время", "Увесь час";
        search: "Search", "Поиск", "Пошук";
        col_name: "Name", "Название", "Назва";
        col_category: "Category", "Категория", "Категорія";
        col_time: "Time", "Время", "Час";
        col_share: "Share", "Доля", "Частка";
        col_sessions: "Sessions", "Сессии", "Сесії";
        col_last: "Last used", "Последний раз", "Востаннє";
        kind_site: "Site", "Сайт", "Сайт";
        kind_app: "App", "Программа", "Програма";
        show_hidden: "Show hidden", "Показать скрытые", "Показати приховані";
        hide: "Hide", "Скрыть", "Сховати";
        unhide: "Show again", "Показать снова", "Показати знову";
        rename: "Rename", "Переименовать", "Перейменувати";
        rename_hint: "Name", "Название", "Назва";
        apps_hint: "Click a row to see its days. Change a category in the list.", "Нажмите на строку, чтобы увидеть её по дням. Категорию можно менять прямо в списке.", "Натисніть на рядок, щоб побачити його за днями. Категорію можна змінювати просто в списку.";
        apps_empty: "Nothing in this period", "За этот период ничего нет", "За цей період нічого немає";
        per_day_of: "Time per day: {name}", "Время по дням: {name}", "Час за днями: {name}";
        hidden_tag: "hidden", "скрыто", "приховано";
        close: "Close", "Закрыть", "Закрити";
        private_browsing: "Private browsing", "Приватный просмотр", "Приватний перегляд";
        other_sites: "Other sites", "Другие сайты", "Інші сайти";

        // Categories
        categories_title: "Categories", "Категории", "Категорії";
        categories_hint: "Every program and site belongs to one category. Built-in rules sort them at first; change any of them in Apps and sites.", "Каждая программа и сайт относится к одной категории. Сначала их разносят встроенные правила; любую можно поменять в разделе «Программы и сайты».", "Кожна програма й сайт належить до однієї категорії. Спочатку їх розносять вбудовані правила; будь-яку можна змінити в розділі «Програми та сайти».";
        builtin_title: "Built-in", "Встроенные", "Вбудовані";
        custom_title: "Your categories", "Ваши категории", "Ваші категорії";
        add_category: "Add category", "Добавить категорию", "Додати категорію";
        category_name: "Category name", "Название категории", "Назва категорії";
        colour: "Colour", "Цвет", "Колір";
        save: "Save", "Сохранить", "Зберегти";
        cancel: "Cancel", "Отмена", "Скасувати";
        delete: "Delete", "Удалить", "Видалити";
        add: "Add", "Добавить", "Додати";
        edit: "Edit", "Изменить", "Змінити";
        no_custom: "No custom categories yet", "Своих категорий пока нет", "Власних категорій поки немає";
        items_in: "{n} items", "элементов: {n}", "елементів: {n}";
        delete_category_hint: "Programs in it move to Other, sites go back to their browser", "Программы из неё перейдут в «Другое», сайты вернутся к своему браузеру", "Програми з неї перейдуть до «Інше», сайти повернуться до свого браузера";

        cat_code: "Code", "Код", "Код";
        cat_browser: "Browser", "Браузер", "Браузер";
        cat_communication: "Communication", "Общение", "Спілкування";
        cat_video: "Video and media", "Видео и медиа", "Відео та медіа";
        cat_games: "Games", "Игры", "Ігри";
        cat_office: "Office and docs", "Офис и документы", "Офіс і документи";
        cat_design: "Design", "Дизайн", "Дизайн";
        cat_system: "System", "Система", "Система";
        cat_other: "Other", "Другое", "Інше";
        rule_code: "Code editors and IDEs, terminals running vim", "Редакторы кода и IDE, терминалы с vim", "Редактори коду та IDE, термінали з vim";
        rule_browser: "Web browsers; sites can have their own category", "Браузеры; у сайтов может быть своя категория", "Браузери; у сайтів може бути своя категорія";
        rule_communication: "Messengers, mail and video calls", "Мессенджеры, почта и видеозвонки", "Месенджери, пошта та відеодзвінки";
        rule_video: "Players, streaming and music", "Плееры, стриминг и музыка", "Плеєри, стрімінг і музика";
        rule_games: "Programs in a game library folder, launchers, and anything full screen that is not otherwise known", "Программы из папок игровых библиотек, лаунчеры и любое неизвестное полноэкранное приложение", "Програми з папок ігрових бібліотек, лаунчери та будь-який невідомий повноекранний застосунок";
        rule_office: "Documents, notes, spreadsheets and PDF readers", "Документы, заметки, таблицы и просмотр PDF", "Документи, нотатки, таблиці та перегляд PDF";
        rule_design: "Graphics, video editing, 3D and audio", "Графика, монтаж, 3D и звук", "Графіка, монтаж, 3D і звук";
        rule_system: "File manager, terminal and settings; can be hidden from charts", "Проводник, терминал и настройки; можно скрыть из диаграмм", "Провідник, термінал і налаштування; можна сховати з діаграм";
        rule_other: "Everything the rules do not know", "Всё, что не знают правила", "Усе, чого не знають правила";

        // Limits
        limits_title: "Limits", "Лимиты", "Ліміти";
        limits_hint: "A daily limit shows a notification when you reach it. It never closes or blocks anything.", "Дневной лимит показывает уведомление, когда вы его достигли. Он ничего не закрывает и не блокирует.", "Денний ліміт показує сповіщення, коли ви його досягли. Він нічого не закриває й не блокує.";
        add_limit: "Add limit", "Добавить лимит", "Додати ліміт";
        limit_for: "For", "Для", "Для";
        limit_daily: "Per day", "В день", "На день";
        limit_kind_category: "Category", "Категория", "Категорія";
        limit_kind_app: "App", "Программа", "Програма";
        used_today: "{used} of {limit} today", "{used} из {limit} сегодня", "{used} з {limit} сьогодні";
        limit_reached: "Limit reached", "Лимит достигнут", "Ліміт досягнуто";
        remove: "Remove", "Убрать", "Прибрати";
        no_limits: "No limits yet", "Лимитов пока нет", "Лімітів поки немає";
        pick_target: "Pick a category or app", "Выберите категорию или программу", "Оберіть категорію або програму";
        limit_notification_title: "Daily limit reached", "Дневной лимит достигнут", "Денний ліміт досягнуто";
        limit_notification_body: "{name}: {limit} today", "{name}: сегодня {limit}", "{name}: сьогодні {limit}";
        minutes_short: "min", "мин", "хв";

        // Settings
        settings_title: "Settings", "Настройки", "Налаштування";
        group_general: "General", "Основные", "Основні";
        group_tracking: "Tracking", "Учёт", "Облік";
        group_data: "Your data", "Ваши данные", "Ваші дані";
        autostart: "Start with Windows", "Запускать вместе с Windows", "Запускати разом із Windows";
        autostart_os: "Start with the computer", "Запускать вместе с компьютером", "Запускати разом із комп'ютером";
        autostart_hint: "Timelyx can only count while it runs, so it starts quietly in the tray", "Timelyx считает время, только пока работает, поэтому он запускается тихо в трее", "Timelyx рахує час, лише поки працює, тому він запускається тихо в треї";
        autostart_failed: "Could not change the startup setting", "Не удалось изменить автозапуск", "Не вдалося змінити автозапуск";
        idle_after: "Stop counting when away", "Не считать, когда вы отошли", "Не рахувати, коли ви відійшли";
        idle_hint: "No keyboard or mouse input for this long. Full screen windows, video and games keep counting", "Нет ввода с клавиатуры и мыши. Полноэкранные окна, видео и игры считаются всегда", "Немає вводу з клавіатури та миші. Повноекранні вікна, відео та ігри рахуються завжди";
        idle_never: "Never", "Никогда", "Ніколи";
        idle_min: "{n} min", "{n} мин", "{n} хв";
        track_sites: "Track sites in browsers", "Учитывать сайты в браузерах", "Враховувати сайти в браузерах";
        track_sites_hint: "Only the site name is stored, never page titles or history", "Хранится только название сайта, но не заголовки страниц и не история", "Зберігається лише назва сайту, а не заголовки сторінок чи історія";
        exclusions: "Excluded sites", "Исключённые сайты", "Виключені сайти";
        exclusions_hint: "Time on these sites counts as the browser only", "Время на этих сайтах учитывается только как браузер", "Час на цих сайтах враховується лише як браузер";
        exclusion_placeholder: "Site name", "Название сайта", "Назва сайту";
        retention: "Keep data", "Хранить данные", "Зберігати дані";
        retention_forever: "Forever", "Всегда", "Завжди";
        retention_months: "{n} months", "{n} мес.", "{n} міс.";
        hide_system: "Hide system apps in charts", "Скрыть системные программы в диаграммах", "Сховати системні програми в діаграмах";
        limit_notifications: "Limit notifications", "Уведомления о лимитах", "Сповіщення про ліміти";
        export_csv: "Export CSV", "Экспорт в CSV", "Експорт у CSV";
        export_hint: "Sessions in a date range, one row each", "Сессии за период, по одной в строке", "Сесії за період, по одній у рядку";
        export_from: "From", "С", "З";
        export_to: "To", "По", "До";
        export_last_7: "Last 7 days", "7 дней", "7 днів";
        export_last_30: "Last 30 days", "30 дней", "30 днів";
        export_all: "All time", "Всё время", "Увесь час";
        export_btn: "Export", "Экспортировать", "Експортувати";
        exported_n: "Exported {n} sessions", "Экспортировано сессий: {n}", "Експортовано сесій: {n}";
        export_failed: "Could not write the file", "Не удалось записать файл", "Не вдалося записати файл";
        open_data_folder: "Open data folder", "Открыть папку данных", "Відкрити теку даних";
        delete_all: "Delete all data", "Удалить все данные", "Видалити всі дані";
        delete_all_hint: "Removes every recorded session. Settings stay", "Удаляет все записанные сессии. Настройки остаются", "Видаляє всі записані сесії. Налаштування лишаються";
        delete_confirm_title: "Delete all recorded data?", "Удалить все записанные данные?", "Видалити всі записані дані?";
        delete_confirm_body: "Every session, program and site is removed from this computer. This cannot be undone", "Все сессии, программы и сайты будут удалены с этого компьютера. Это нельзя отменить", "Усі сесії, програми й сайти буде видалено з цього комп'ютера. Це не можна скасувати";
        delete_do: "Delete everything", "Удалить всё", "Видалити все";
        deleted: "All data deleted", "Все данные удалены", "Усі дані видалено";
        tracker_status: "Tracker", "Учёт", "Облік";
        tracker_running: "Running in the background", "Работает в фоне", "Працює у фоні";
        tracker_stopped: "Not running", "Не запущен", "Не запущено";
        tracker_stop: "Stop tracker", "Остановить учёт", "Зупинити облік";
        week_start: "Week starts on", "Неделя начинается с", "Тиждень починається з";
        monday: "Monday", "понедельника", "понеділка";
        sunday: "Sunday", "воскресенья", "неділі";
        week_fixed_hint: "Weeks start on Monday", "Неделя начинается с понедельника", "Тиждень починається з понеділка";
        time_format: "Time format", "Формат времени", "Формат часу";
        about: "About", "О программе", "Про програму";
        version: "Version", "Версия", "Версія";
        privacy_note: "Everything stays on this computer. Nothing is sent anywhere.", "Всё хранится на этом компьютере. Ничего никуда не отправляется.", "Усе зберігається на цьому комп'ютері. Нічого нікуди не надсилається.";

        // First run
        ob_title: "Welcome to Timelyx", "Добро пожаловать в Timelyx", "Ласкаво просимо до Timelyx";
        ob_lead: "Timelyx tracks which programs and sites you use, and for how long, so you can see where your time goes.", "Timelyx отслеживает, какими программами и сайтами вы пользуетесь и как долго, чтобы вы видели, куда уходит время.", "Timelyx відстежує, якими програмами та сайтами ви користуєтеся і як довго, щоб ви бачили, куди йде час.";
        ob_recorded: "What is recorded", "Что записывается", "Що записується";
        ob_rec_1: "The program in front of you and when you switch", "Программа, которая у вас на переднем плане, и моменты переключения", "Програма, яка у вас на передньому плані, і моменти перемикання";
        ob_rec_2: "In browsers, only the site name such as YouTube, never page titles or history", "В браузерах только название сайта, например YouTube, без заголовков страниц и истории", "У браузерах лише назва сайту, наприклад YouTube, без заголовків сторінок та історії";
        ob_rec_3: "Time away from the keyboard is not counted", "Время, когда вы отошли от клавиатуры, не считается", "Час, коли ви відійшли від клавіатури, не рахується";
        ob_local: "Everything stays on this computer. Nothing is sent anywhere.", "Всё хранится на этом компьютере. Ничего никуда не отправляется.", "Усе зберігається на цьому комп'ютері. Нічого нікуди не надсилається.";
        ob_autostart_note: "Timelyx starts with your computer and runs quietly in the tray, because it can only count while it runs. You can turn this off now or later in Settings.", "Timelyx запускается вместе с компьютером и тихо работает в трее, потому что считать время он может, только пока работает. Это можно отключить сейчас или позже в настройках.", "Timelyx запускається разом із комп'ютером і тихо працює в треї, бо рахувати час він може, лише поки працює. Це можна вимкнути зараз або пізніше в налаштуваннях.";
        ob_start: "Start tracking", "Начать учёт", "Почати облік";

        // Tray and notifications
        tray_open: "Open Timelyx", "Открыть Timelyx", "Відкрити Timelyx";
        tray_pause: "Pause tracking", "Приостановить учёт", "Призупинити облік";
        tray_pause_15: "For 15 minutes", "На 15 минут", "На 15 хвилин";
        pause_15: "Pause for 15 minutes", "Пауза на 15 минут", "Пауза на 15 хвилин";
        tray_pause_60: "For 1 hour", "На 1 час", "На 1 годину";
        tray_pause_until: "Until I resume", "Пока я не включу", "Доки я не ввімкну";
        tray_resume: "Resume tracking", "Продолжить учёт", "Продовжити облік";
        tray_exit: "Exit", "Выход", "Вихід";
        tray_today: "Today: {time}", "Сегодня: {time}", "Сьогодні: {time}";
        tray_paused: "paused", "пауза", "пауза";
        tray_unavailable: "tracking unavailable", "учёт недоступен", "облік недоступний";
    }
}

/// Name of a built-in category in the given language; `None` for custom ids.
pub fn builtin_name(lang: crate::i18n::Lang, id: &str) -> Option<&'static str> {
    let t = TABLE.get(lang);
    Some(match id {
        crate::cats::CODE => t.cat_code,
        crate::cats::BROWSER => t.cat_browser,
        crate::cats::COMMUNICATION => t.cat_communication,
        crate::cats::VIDEO => t.cat_video,
        crate::cats::GAMES => t.cat_games,
        crate::cats::OFFICE => t.cat_office,
        crate::cats::DESIGN => t.cat_design,
        crate::cats::SYSTEM => t.cat_system,
        crate::cats::OTHER => t.cat_other,
        _ => return None,
    })
}

/// What a built-in category contains, as one line.
pub fn builtin_rule(lang: crate::i18n::Lang, id: &str) -> Option<&'static str> {
    let t = TABLE.get(lang);
    Some(match id {
        crate::cats::CODE => t.rule_code,
        crate::cats::BROWSER => t.rule_browser,
        crate::cats::COMMUNICATION => t.rule_communication,
        crate::cats::VIDEO => t.rule_video,
        crate::cats::GAMES => t.rule_games,
        crate::cats::OFFICE => t.rule_office,
        crate::cats::DESIGN => t.rule_design,
        crate::cats::SYSTEM => t.rule_system,
        crate::cats::OTHER => t.rule_other,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Lang;

    #[test]
    fn no_empty_strings_and_no_trailing_period_on_labels() {
        for lang in Lang::ALL {
            let t = TABLE.get(lang);
            for s in [t.nav_today, t.nav_settings, t.tray_open, t.ob_start, t.delete_do, t.cat_other, t.rule_other] {
                assert!(!s.is_empty());
            }
            for s in [t.nav_today, t.nav_trends, t.col_name, t.add, t.save, t.tray_exit, t.autostart, t.start_tracking] {
                assert!(!s.ends_with('.') && !s.contains('!'), "{s}");
            }
        }
    }

    #[test]
    fn every_builtin_category_has_a_name_and_a_rule_in_every_language() {
        for lang in Lang::ALL {
            for (id, _) in crate::cats::BUILTIN {
                assert!(builtin_name(lang, id).is_some_and(|n| !n.is_empty()), "{id}");
                assert!(builtin_rule(lang, id).is_some_and(|n| !n.is_empty()), "{id}");
            }
        }
        assert!(builtin_name(Lang::En, "c1").is_none());
    }

    #[test]
    fn placeholders_survive_translation() {
        for lang in Lang::ALL {
            let t = TABLE.get(lang);
            assert!(t.tray_today.contains("{time}") && t.sessions_n.contains("{n}") && t.items_in.contains("{n}"));
            assert!(t.used_today.contains("{used}") && t.used_today.contains("{limit}"));
            assert!(t.limit_notification_body.contains("{name}") && t.limit_notification_body.contains("{limit}"));
            assert!(t.idle_min.contains("{n}") && t.retention_months.contains("{n}") && t.exported_n.contains("{n}"));
            assert!(t.per_day_of.contains("{name}") && t.now_paused_until.contains("{time}"));
        }
    }
}
