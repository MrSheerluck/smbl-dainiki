import datetime as dt
import os
import platform
import tempfile
import unittest
import zipfile

from playwright.sync_api import sync_playwright

BASE_URL = os.environ.get("DAINIKI_URL", "http://127.0.0.1:3000")
MOD = "Meta" if platform.system() == "Darwin" else "Control"


def iso_shift(iso: str, days: int) -> str:
    return (dt.date.fromisoformat(iso) + dt.timedelta(days=days)).isoformat()


class DainikiE2E(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.playwright = sync_playwright().start()
        cls.browser = cls.playwright.chromium.launch()

    @classmethod
    def tearDownClass(cls):
        cls.browser.close()
        cls.playwright.stop()

    def setUp(self):
        self.context = self.browser.new_context(
            viewport={"width": 1280, "height": 1000},
            accept_downloads=True,
        )
        self.page = self.context.new_page()
        self.page_errors = []
        self.page.on("pageerror", lambda error: self.page_errors.append(str(error)))
        self.open_app()

    def tearDown(self):
        self.context.close()
        self.assertEqual(self.page_errors, [])

    def wait_for_editor(self):
        for _ in range(2):
            try:
                self.page.wait_for_selector("#entry-editor .line", timeout=10000)
                self.page.wait_for_timeout(700)

                if self.page.locator("#entry-editor .line").count() > 0:
                    return
            except Exception:
                pass

            self.page.reload(wait_until="networkidle")

        self.page.wait_for_selector("#entry-editor .line", timeout=15000)

    def open_app(self):
        self.page.goto(BASE_URL, wait_until="networkidle")
        self.wait_for_editor()

        if self.page.locator("#welcome-overlay").is_visible():
            self.page.locator("#start-writing").click()
            self.page.wait_for_timeout(300)

    def today(self) -> str:
        return self.page.evaluate(
            """() => {
                const d = new Date();
                const p = (n) => String(n).padStart(2, "0");
                return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
            }"""
        )

    def seed(self, entries):
        self.page.evaluate(
            """async (entries) => {
                const root = await navigator.storage.getDirectory();
                const journal = await root.getDirectoryHandle("journal", { create: true });

                for (const entry of entries) {
                    const [year, month] = entry.iso.split("-");
                    const yearDir = await journal.getDirectoryHandle(year, { create: true });
                    const monthDir = await yearDir.getDirectoryHandle(month, { create: true });
                    const file = await monthDir.getFileHandle(`${entry.iso}.md`, { create: true });
                    const writable = await file.createWritable();
                    await writable.write(entry.text);
                    await writable.close();
                }
            }""",
            entries,
        )

    def read_entry(self, iso: str) -> str:
        return self.page.evaluate(
            """async (iso) => {
                const root = await navigator.storage.getDirectory();
                const journal = await root.getDirectoryHandle("journal");
                const [year, month] = iso.split("-");
                const yearDir = await journal.getDirectoryHandle(year);
                const monthDir = await yearDir.getDirectoryHandle(month);
                const file = await (await monthDir.getFileHandle(`${iso}.md`)).getFile();
                return await file.text();
            }""",
            iso,
        )

    def list_files(self):
        return self.page.evaluate(
            """async () => {
                const root = await navigator.storage.getDirectory();
                const journal = await root.getDirectoryHandle("journal");
                const names = [];
                for await (const year of journal.values()) {
                    if (year.kind !== "directory") continue;
                    for await (const month of year.values()) {
                        if (month.kind !== "directory") continue;
                        for await (const file of month.values()) {
                            if (file.kind === "file") names.push(file.name);
                        }
                    }
                }
                return names;
            }"""
        )

    def type_today(self, text: str):
        if self.page.locator("#entry-editor .line").count() == 0:
            self.wait_for_editor()

        self.page.locator("#entry-editor .line").nth(0).click()
        self.page.keyboard.type(text)
        self.page.wait_for_timeout(900)

    def test_first_run_welcome(self):
        context = self.browser.new_context()
        page = context.new_page()

        try:
            page.goto(BASE_URL, wait_until="networkidle")
            page.wait_for_selector("#welcome-overlay", state="visible", timeout=15000)
            page.locator("#start-writing").click()
            page.wait_for_timeout(500)
            self.assertFalse(page.locator("#welcome-overlay").is_visible())
            self.assertEqual(page.evaluate("localStorage.getItem('dainiki-onboarded')"), "true")
        finally:
            context.close()

    def test_write_and_reload(self):
        today = self.today()
        self.type_today("A quiet test entry. #test")
        self.assertIn("Saved locally", self.page.locator("#entry-status").inner_text())
        self.assertEqual(self.read_entry(today), "A quiet test entry. #test")

        self.page.reload(wait_until="networkidle")
        self.page.wait_for_timeout(900)
        self.assertIn("A quiet test entry.", self.page.locator("#entry-editor").inner_text())
        self.assertEqual(self.page.locator("#today-day").inner_text(), str(dt.date.today().day))

    def test_inline_past_entry_editing(self):
        today = self.today()
        yesterday = iso_shift(today, -1)
        before = iso_shift(today, -2)

        self.seed(
            [
                {"iso": yesterday, "text": "Yesterday text. #past"},
                {"iso": before, "text": "Two days ago."},
            ]
        )
        self.page.reload(wait_until="networkidle")
        self.page.wait_for_timeout(900)

        self.page.locator(f'[data-entry-card][data-iso="{yesterday}"]').click()
        self.page.wait_for_timeout(500)
        self.assertEqual(self.page.locator("#timeline-entries .inline-live").count(), 1)
        self.assertEqual(self.page.locator("#timeline-entries .inline-live .line").count(), 1)
        self.assertEqual(self.page.locator("#timeline-entries .inline-live .line.active").count(), 1)

        self.page.locator("#timeline-entries .inline-live .line").first.click()
        self.page.keyboard.press("End")
        self.page.keyboard.type(" edited.")
        self.page.wait_for_timeout(900)
        self.assertEqual(self.read_entry(yesterday), "Yesterday text. #past edited.")

        self.page.locator("#entry-editor .line").first.click()
        self.page.wait_for_timeout(800)
        self.assertIn("Yesterday text.", self.page.locator(f'[data-entry-card][data-iso="{yesterday}"]').inner_text())
        self.assertIn("edited.", self.page.locator(f'[data-entry-card][data-iso="{yesterday}"]').inner_text())
        self.assertEqual(self.page.locator("#today-day").inner_text(), str(dt.date.today().day))

    def test_inline_live_preview_renders_inactive_lines(self):
        yesterday = iso_shift(self.today(), -1)
        self.seed([{"iso": yesterday, "text": "First **bold** line\nSecond [link](https://example.com) line"}])
        self.page.reload(wait_until="networkidle")
        self.page.wait_for_timeout(900)

        timeline_text = self.page.locator("#timeline-entries").inner_text()
        self.assertIn("First bold line", timeline_text)
        self.assertIn("Second link line", timeline_text)
        self.assertEqual(self.page.locator("#timeline-entries a[data-entry-card]").count(), 0)
        self.assertGreaterEqual(self.page.locator("#timeline-entries [data-entry-card] .md-link").count(), 1)

        self.page.locator(f'[data-entry-card][data-iso="{yesterday}"]').click()
        self.page.wait_for_timeout(500)
        self.assertEqual(self.page.locator("#timeline-entries .inline-live .line").count(), 2)
        self.assertEqual(self.page.locator("#timeline-entries .inline-live .md-strong").count(), 1)
        self.assertEqual(self.page.locator("#timeline-entries .inline-live .line.active").count(), 1)
        self.assertNotIn("**bold**", self.page.locator("#timeline-entries .inline-live").inner_text())

        self.page.locator("#timeline-entries .inline-live .line").first.click()
        self.page.wait_for_timeout(200)
        self.assertIn("**bold**", self.page.locator("#timeline-entries .inline-live .line.active").inner_text())

    def test_search_opens_inline(self):
        today = self.today()
        yesterday = iso_shift(today, -1)

        self.seed([{"iso": yesterday, "text": "A quiet morning with tea. #morning"}])
        self.page.reload(wait_until="networkidle")
        self.page.wait_for_timeout(900)

        self.page.keyboard.press(f"{MOD}+k")
        self.page.wait_for_timeout(300)
        self.page.keyboard.type("quiet morning")
        self.page.wait_for_timeout(400)
        self.assertGreaterEqual(self.page.locator("#search-results button").count(), 1)

        self.page.keyboard.press("Enter")
        self.page.wait_for_timeout(900)
        self.assertEqual(self.page.locator("#timeline-entries .inline-live").count(), 1)
        self.assertEqual(self.page.locator("#today-day").inner_text(), str(dt.date.today().day))

    def test_archive_jumps_across_years_months_and_days(self):
        today = dt.date.today()
        old = dt.date(today.year - 2, 3, 14)
        last_year = dt.date(today.year - 1, 7, 4)
        self.seed(
            [
                {"iso": old.isoformat(), "text": "A spring day years ago. #archive"},
                {"iso": last_year.isoformat(), "text": "A summer entry."},
            ]
        )
        self.page.reload(wait_until="networkidle")
        self.page.wait_for_timeout(900)

        self.assertEqual(self.page.locator("#prev-day").count(), 0)
        self.assertEqual(self.page.locator("#next-day").count(), 0)

        self.page.locator("#open-archive").click()
        self.page.wait_for_timeout(300)
        self.assertTrue(self.page.locator("#archive-overlay").is_visible())
        self.assertIn("Browse the journal", self.page.locator("#archive-overlay").inner_text())

        self.page.locator(f'[data-archive-year="{old.year}"]').click()
        self.page.locator('[data-archive-month="3"]').click()
        self.page.locator(f'[data-archive-day="{old.isoformat()}"]').click()
        self.page.wait_for_timeout(800)

        self.assertFalse(self.page.locator("#archive-overlay").is_visible())
        self.assertEqual(self.page.locator("#today-day").inner_text(), str(old.day))
        self.assertIn("A spring day years ago.", self.page.locator("#entry-editor").inner_text())

        self.page.locator("#open-archive").click()
        self.page.wait_for_timeout(200)
        self.page.locator("[data-archive-today]").click()
        self.page.wait_for_timeout(700)
        self.assertEqual(
            self.page.locator("#today-day").inner_text(),
            str(today.day),
        )

    def test_tags_filter_and_calendar(self):
        today = self.today()
        yesterday = iso_shift(today, -1)
        before = iso_shift(today, -2)

        self.seed(
            [
                {"iso": yesterday, "text": "Morning one. #morning"},
                {"iso": before, "text": "Morning two. #morning #work"},
            ]
        )
        self.page.reload(wait_until="networkidle")
        self.page.wait_for_timeout(900)

        self.assertIn("morning", self.page.locator("#tag-list").inner_text())
        self.page.locator('#tag-list button:has-text("morning")').click()
        self.page.wait_for_timeout(300)
        self.assertTrue(self.page.locator("#active-filter").is_visible())

        self.page.locator("#clear-filter").click()
        self.page.wait_for_timeout(200)
        self.assertFalse(self.page.locator("#active-filter").is_visible())

        self.page.locator(f'#calendar button[aria-label="{yesterday}"]').click()
        self.page.wait_for_timeout(700)
        self.assertEqual(self.page.locator("#timeline-entries .inline-live").count(), 1)
        self.assertEqual(self.page.locator("#today-day").inner_text(), str(dt.date.today().day))

    def test_sidebar_search_and_settings(self):
        self.page.locator("#open-search-sidebar").click()
        self.page.wait_for_timeout(300)
        self.assertTrue(self.page.locator("#search-overlay").is_visible())
        self.page.keyboard.press("Escape")
        self.page.wait_for_timeout(200)

        self.page.locator("#open-settings").click()
        self.page.wait_for_timeout(300)
        self.assertTrue(self.page.locator("#settings-overlay").is_visible())
        self.page.locator('[data-setting-theme="dark"]').click()
        self.page.locator('[data-setting-size="large"]').click()
        self.page.locator("#setting-spellcheck").uncheck()
        self.page.wait_for_timeout(500)

        self.assertEqual(self.page.evaluate("document.documentElement.dataset.theme"), "dark")
        self.assertEqual(
            self.page.evaluate("document.documentElement.dataset.editorSize"),
            "large",
        )
        self.assertEqual(
            self.page.locator("#entry-editor").get_attribute("spellcheck"),
            "false",
        )

        self.page.reload(wait_until="networkidle")
        self.page.wait_for_timeout(800)
        self.assertEqual(self.page.evaluate("document.documentElement.dataset.theme"), "dark")
        self.assertEqual(
            self.page.evaluate("document.documentElement.dataset.editorSize"),
            "large",
        )
        self.assertEqual(
            self.page.locator("#entry-editor").get_attribute("spellcheck"),
            "false",
        )

        self.page.locator("#open-settings").click()
        self.page.wait_for_timeout(200)
        self.page.locator('[data-settings-action="archive"]').click()
        self.page.wait_for_timeout(300)
        self.assertTrue(self.page.locator("#archive-overlay").is_visible())

    def test_zen_mode(self):
        self.type_today("A quiet room for attention.")
        self.page.locator("#toggle-zen").click()
        self.page.wait_for_timeout(800)
        self.assertEqual(self.page.evaluate("document.documentElement.dataset.zen"), "true")
        self.assertTrue(self.page.locator("#exit-zen").is_visible())
        self.assertEqual(
            self.page.locator(".app-sidebar").evaluate(
                "(el) => getComputedStyle(el).marginLeft"
            ),
            "-230px",
        )
        self.assertNotEqual(
            self.page.locator(".journal-page").evaluate(
                "(el) => getComputedStyle(el).backgroundColor"
            ),
            "rgba(0, 0, 0, 0)",
        )

        self.page.keyboard.press(f"{MOD}+.")
        self.page.wait_for_timeout(700)
        self.assertIsNone(self.page.evaluate("document.documentElement.dataset.zen"))

        self.page.locator("#toggle-zen").click()
        self.page.wait_for_timeout(700)
        self.page.reload(wait_until="networkidle")
        self.page.wait_for_timeout(800)
        self.assertEqual(self.page.evaluate("document.documentElement.dataset.zen"), "true")
        self.page.keyboard.press("Escape")
        self.page.wait_for_timeout(700)
        self.assertIsNone(self.page.evaluate("document.documentElement.dataset.zen"))

    def test_theme_persists(self):
        self.page.locator("#theme-toggle").click()
        self.page.wait_for_timeout(300)
        theme = self.page.evaluate("document.documentElement.dataset.theme")
        self.assertIn(theme, ("light", "dark"))

        self.page.reload(wait_until="networkidle")
        self.page.wait_for_timeout(700)
        self.assertEqual(self.page.evaluate("document.documentElement.dataset.theme"), theme)

    def test_conflict_copy(self):
        today = self.today()
        self.type_today("original")
        self.page.wait_for_timeout(300)

        self.page.evaluate(
            """async (iso) => {
                const root = await navigator.storage.getDirectory();
                const journal = await root.getDirectoryHandle("journal");
                const [year, month] = iso.split("-");
                const yearDir = await journal.getDirectoryHandle(year);
                const monthDir = await yearDir.getDirectoryHandle(month);
                const file = await monthDir.getFileHandle(`${iso}.md`);
                const writable = await file.createWritable();
                await writable.write("external");
                await writable.close();
            }""",
            today,
        )

        self.page.locator("#entry-editor .line").nth(0).click()
        self.page.keyboard.press("End")
        self.page.keyboard.type(" local")
        self.page.wait_for_timeout(1100)

        self.assertIn("Conflict copy", self.page.locator("#entry-status").inner_text())
        files = self.list_files()
        self.assertTrue(any(".conflict-" in name for name in files), files)
        self.assertEqual(self.read_entry(today), "external")

        self.assertTrue(self.page.locator("#open-conflicts").is_visible())
        self.assertEqual(self.page.locator("#conflict-count").inner_text(), "1")
        self.page.locator("#open-conflicts").click()
        self.page.wait_for_timeout(300)
        self.assertIn("original local", self.page.locator("#conflict-content").inner_text())
        self.assertIn("external", self.page.locator("#conflict-content").inner_text())

        self.page.on("dialog", lambda dialog: dialog.accept())
        self.page.locator('[data-conflict-action="use"]').click()
        self.page.wait_for_timeout(800)
        self.assertFalse(self.page.locator("#open-conflicts").is_visible())
        self.assertEqual(self.read_entry(today), "original local")

    def test_export_and_import(self):
        today = self.today()
        yesterday = iso_shift(today, -1)
        self.seed(
            [
                {"iso": today, "text": "Today export. #export"},
                {"iso": yesterday, "text": "Yesterday export. #export"},
            ]
        )
        self.page.evaluate(
            """async (iso) => {
                const root = await navigator.storage.getDirectory();
                const journal = await root.getDirectoryHandle("journal");
                const [year, month] = iso.split("-");
                const yearDir = await journal.getDirectoryHandle(year);
                const monthDir = await yearDir.getDirectoryHandle(month);
                const file = await monthDir.getFileHandle(
                    `${iso}.conflict-123.md`,
                    {create: true}
                );
                const writable = await file.createWritable();
                await writable.write("Exported conflict copy.");
                await writable.close();
            }""",
            today,
        )
        self.page.reload(wait_until="networkidle")
        self.page.wait_for_timeout(900)

        self.page.locator("#open-data").click()
        self.page.wait_for_timeout(200)
        with self.page.expect_download() as download_info:
            self.page.locator("#export-zip").click()

        download = download_info.value
        path = os.path.join(tempfile.gettempdir(), "dainiki-e2e-export.zip")
        download.save_as(path)

        with zipfile.ZipFile(path) as archive:
            names = archive.namelist()
            self.assertIn(f"{today[:4]}/{today[5:7]}/{today}.md", names)
            self.assertIn(
                f"{today[:4]}/{today[5:7]}/{today}.conflict-123.md", names
            )
            self.assertIn("README.md", names)

        self.page.evaluate(
            """async () => {
                const root = await navigator.storage.getDirectory();
                await root.removeEntry("journal", { recursive: true });
            }"""
        )
        self.page.reload(wait_until="networkidle")
        self.page.wait_for_timeout(800)

        self.page.locator("#open-data").click()
        self.page.wait_for_timeout(200)
        self.page.locator("#import-file").set_input_files(path)
        self.page.wait_for_timeout(1800)

        self.assertIn("Imported", self.page.locator("#data-status").inner_text())
        self.assertIn("Today export.", self.page.locator("#entry-editor").inner_text())
        self.assertGreaterEqual(self.page.locator("#timeline-entries article").count(), 1)
        self.assertTrue(
            any(".conflict-123.md" in name for name in self.list_files()),
            self.list_files(),
        )
        self.assertTrue(self.page.locator("#open-conflicts").is_visible())

    def test_offline_shell(self):
        self.page.reload(wait_until="networkidle")
        self.page.wait_for_timeout(900)
        self.page.wait_for_function(
            "navigator.serviceWorker.controller !== null",
            timeout=15000,
        )
        self.page.reload(wait_until="networkidle")
        self.page.wait_for_timeout(900)
        self.assertTrue(self.page.evaluate("!!navigator.serviceWorker.controller"))

        self.context.set_offline(True)
        self.page.reload(wait_until="domcontentloaded")
        self.page.wait_for_timeout(1500)
        self.assertEqual(self.page.locator("#entry-editor").count(), 1)
        self.context.set_offline(False)

    def test_mobile_drawer(self):
        self.page.set_viewport_size({"width": 390, "height": 844})
        self.page.reload(wait_until="networkidle")
        self.page.wait_for_timeout(700)

        self.assertFalse(self.page.locator("aside").is_visible())
        self.page.locator("#open-sidebar").click()
        self.page.wait_for_timeout(300)
        self.assertTrue(self.page.locator("aside").is_visible())
        self.assertTrue(self.page.locator("#sidebar-backdrop").is_visible())

        self.page.locator("#close-sidebar").click()
        self.page.wait_for_timeout(300)
        self.assertFalse(self.page.locator("aside").is_visible())


if __name__ == "__main__":
    unittest.main()
