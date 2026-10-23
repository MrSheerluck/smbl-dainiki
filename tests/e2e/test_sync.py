import os
import re
import time
import unittest
from datetime import date, timedelta
from pathlib import Path

from playwright.sync_api import sync_playwright

BASE_URL = os.environ.get("DAINIKI_URL", "http://127.0.0.1:3001")
OUTBOX = Path(
    os.environ.get(
        "DAINIKI_OUTBOX",
        Path(__file__).resolve().parents[2] / "data" / "outbox",
    )
)
PASSWORD = "correct horse battery staple"
SYNC_PASSPHRASE = "a quiet passphrase for encrypted sync"


class DainikiSyncE2E(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.playwright = sync_playwright().start()
        cls.browser = cls.playwright.chromium.launch()

    @classmethod
    def tearDownClass(cls):
        cls.browser.close()
        cls.playwright.stop()

    def setUp(self):
        self.contexts = []
        self.page_errors = []
        self.page = self.new_page()

    def tearDown(self):
        for context in self.contexts:
            context.close()
        self.assertEqual(self.page_errors, [])

    def new_page(self):
        context = self.browser.new_context(
            viewport={"width": 1280, "height": 1200},
            accept_downloads=True,
        )
        self.contexts.append(context)
        page = context.new_page()
        page.on("pageerror", lambda error: self.page_errors.append(str(error)))
        return page

    def email(self):
        return f"sync+{time.time_ns()}@example.com"

    def goto(self, page, path):
        page.goto(f"{BASE_URL}{path}", wait_until="networkidle")

    def signup(self, page, email):
        self.goto(page, "/signup")
        page.fill("#signup-email", email)
        page.fill("#signup-password", PASSWORD)
        page.fill("#signup-confirm", PASSWORD)
        page.click("#signup-submit")
        page.wait_for_load_state("networkidle")

    def verify(self, page, email):
        safe = email.replace("+", "_")
        files = sorted(OUTBOX.glob(f"*{safe}*.eml"))
        self.assertTrue(files, f"no verification email for {email}")
        match = re.search(
            r"https?://[^\s]+/verify-email\?token=[A-Za-z0-9_-]+",
            files[-1].read_text(),
        )
        self.assertIsNotNone(match)
        self.goto(page, match.group(0).removeprefix(BASE_URL))
        self.assertEqual(page.locator("h1").inner_text(), "Email verified")

    def activate(self, page, email):
        self.signup(page, email)
        self.verify(page, email)
        self.goto(page, "/billing")
        if page.locator("text=Local test billing is on.").count() == 0:
            self.skipTest("sync tests need the mock billing server")
        page.click('button:has-text("Subscribe monthly")')
        page.wait_for_load_state("networkidle")
        self.assertIn("mock-active", page.url)

    def signin(self, page, email):
        self.goto(page, "/signin")
        page.fill("#signin-email", email)
        page.fill("#signin-password", PASSWORD)
        page.click("#signin-submit")
        page.wait_for_load_state("networkidle")

    def open_journal(self, page):
        self.goto(page, "/")
        page.wait_for_timeout(700)
        if page.locator("#welcome-overlay").is_visible():
            page.click("#start-writing")
            page.wait_for_timeout(300)
        page.wait_for_selector("#open-sync", timeout=15000)

    def seed(self, page, entries):
        page.evaluate(
            """async (entries) => {
                const root = await navigator.storage.getDirectory();
                const journal = await root.getDirectoryHandle("journal", {create: true});
                for (const entry of entries) {
                    const [year, month] = entry.iso.split("-");
                    const yearDir = await journal.getDirectoryHandle(year, {create: true});
                    const monthDir = await yearDir.getDirectoryHandle(month, {create: true});
                    const file = await monthDir.getFileHandle(entry.iso + ".md", {create: true});
                    const writable = await file.createWritable();
                    await writable.write(entry.text);
                    await writable.close();
                }
            }""",
            entries,
        )
        page.reload(wait_until="networkidle")
        page.wait_for_timeout(700)

    def open_sync(self, page):
        if not page.locator("#sync-overlay").is_visible():
            page.click("#open-sync")

        deadline = time.time() + 15
        while time.time() < deadline:
            text = page.locator("#sync-content").inner_text()
            if "Loading" not in text:
                page.wait_for_timeout(150)
                return
            page.wait_for_timeout(200)

    def click_sync_now(self, page):
        before = page.locator("#sync-content").inner_text()
        page.click('[data-sync-action="sync-now"]')
        deadline = time.time() + 90
        while time.time() < deadline:
            text = page.locator("#sync-content").inner_text()
            if text != before and ("Synced" in text or "changed" in text):
                return text
            page.wait_for_timeout(500)
        return page.locator("#sync-content").inner_text()

    def wait_for_sync(self, page, timeout=90):
        deadline = time.time() + timeout
        while time.time() < deadline:
            text = page.locator("#sync-content").inner_text()
            if "Synced" in text or "changed" in text or "failed" in text.lower():
                return text
            page.wait_for_timeout(500)
        return page.locator("#sync-content").inner_text()

    def setup_sync(self, page):
        self.open_sync(page)
        self.assertIn("Your plan includes encrypted sync", page.locator("#sync-content").inner_text())
        page.fill("#sync-passphrase", SYNC_PASSPHRASE)
        page.fill("#sync-confirm", SYNC_PASSPHRASE)
        page.click('[data-sync-action="setup"]')

        deadline = time.time() + 90
        clicked = False
        while time.time() < deadline:
            text = page.locator("#sync-content").inner_text()
            if "Synced" in text:
                return text
            if not clicked and page.locator('[data-sync-action="sync-now"]').count():
                page.click('[data-sync-action="sync-now"]')
                clicked = True
            page.wait_for_timeout(500)

        return page.locator("#sync-content").inner_text()

    def unlock_sync(self, page):
        self.open_sync(page)
        page.fill("#sync-passphrase", SYNC_PASSPHRASE)
        page.click('[data-sync-action="unlock"]')
        return self.wait_for_sync(page)

    def local_files(self, page):
        return page.evaluate(
            """async () => {
                const root = await navigator.storage.getDirectory();
                const journal = await root.getDirectoryHandle("journal");
                const files = [];
                for await (const year of journal.values()) {
                    if (year.kind !== "directory") continue;
                    for await (const month of year.values()) {
                        if (month.kind !== "directory") continue;
                        for await (const file of month.values()) files.push(file.name);
                    }
                }
                return files;
            }"""
        )

    def test_sync_requires_active_plan(self):
        email = self.email()
        self.signup(self.page, email)
        self.verify(self.page, email)
        self.open_journal(self.page)
        self.assertEqual(self.page.locator("#sync-label").inner_text().upper(), "SIGNED IN")
        self.assertIn("No active plan", self.page.locator("#sync-detail").inner_text())
        self.open_sync(self.page)
        self.assertIn("needs an active plan", self.page.locator("#sync-content").inner_text())

    def test_setup_two_device_auto_sync(self):
        email = self.email()
        self.activate(self.page, email)
        self.open_journal(self.page)
        today = time.strftime("%Y-%m-%d")
        self.seed(
            self.page,
            [
                {"iso": today, "text": "encrypted from device one"},
                {"iso": "2026-10-22", "text": "older encrypted entry"},
            ],
        )

        panel = self.setup_sync(self.page)
        self.assertIn("Synced 2 up", panel)
        self.assertIn("recovery key", self.page.locator("#sync-content").inner_text().lower())
        self.assertEqual(self.page.locator("#sync-label").inner_text().upper(), "PAID SYNC")
        self.assertIn("On ·", self.page.locator("#sync-detail").inner_text())

        device_two = self.new_page()
        self.signin(device_two, email)
        self.open_journal(device_two)
        self.assertIn("Connect this device", device_two.locator("#sync-detail").inner_text())
        panel = self.unlock_sync(device_two)
        self.assertIn("2 down", panel)
        self.assertIn("encrypted from device one", device_two.locator("#entry-editor").inner_text())
        self.assertTrue(any(name.endswith(".md") for name in self.local_files(device_two)))

        # The device remembers the key, so a reload syncs automatically.
        device_two.evaluate(
            """async () => {
                const root = await navigator.storage.getDirectory();
                try { await root.removeEntry("journal", {recursive: true}); } catch (error) {}
            }"""
        )
        device_two.reload(wait_until="networkidle")
        deadline = time.time() + 30
        while time.time() < deadline:
            if (
                "On ·" in device_two.locator("#sync-detail").inner_text()
                and "encrypted from device one"
                in device_two.locator("#entry-editor").inner_text()
            ):
                break
            device_two.wait_for_timeout(500)
        self.assertIn("encrypted from device one", device_two.locator("#entry-editor").inner_text())
        self.assertEqual(device_two.locator('[data-sync-action="lock"]').count(), 0)

        self.open_sync(device_two)
        device_two.on("dialog", lambda dialog: dialog.accept())
        device_two.click('[data-sync-action="forget"]')
        device_two.wait_for_timeout(500)
        self.assertIn("Connect this device", device_two.locator("#sync-detail").inner_text())
        self.assertIn("Enter your sync passphrase", device_two.locator("#sync-content").inner_text())

    def test_conflict_copy_and_remote_delete(self):
        email = self.email()
        self.activate(self.page, email)
        self.open_journal(self.page)
        today = time.strftime("%Y-%m-%d")
        self.seed(self.page, [{"iso": today, "text": "shared baseline"}])
        self.assertIn("Synced 1 up", self.setup_sync(self.page))

        device_two = self.new_page()
        self.signin(device_two, email)
        self.open_journal(device_two)
        self.assertIn("1 down", self.unlock_sync(device_two))

        self.page.click("#close-sync")
        self.page.locator("#entry-editor .line").nth(0).click()
        self.page.keyboard.press("End")
        self.page.keyboard.type(" alpha")
        self.page.wait_for_timeout(800)
        self.open_sync(self.page)
        self.assertIn("Synced", self.click_sync_now(self.page))

        device_two.click("#close-sync")
        device_two.locator("#entry-editor .line").nth(0).click()
        device_two.keyboard.press("End")
        device_two.keyboard.type(" beta")
        device_two.wait_for_timeout(800)
        self.open_sync(device_two)
        panel = self.click_sync_now(device_two)
        self.assertIn("conflict copies saved", panel)
        self.assertIn("shared baseline alpha", device_two.locator("#entry-editor").inner_text())
        self.assertTrue(
            any(".conflict-" in name for name in self.local_files(device_two)),
            self.local_files(device_two),
        )

        device_two.click("#close-sync")
        device_two.click("#open-data")
        device_two.on("dialog", lambda dialog: dialog.accept())
        device_two.click("#delete-entry")
        device_two.wait_for_timeout(3500)
        self.open_sync(device_two)
        device_two.wait_for_timeout(500)
        self.assertIn("deleted", device_two.locator("#sync-content").inner_text())

        self.click_sync_now(self.page)
        self.assertEqual(self.page.locator("#entry-editor").inner_text().strip(), "")
        self.assertNotIn("shared baseline", self.page.locator("#timeline-entries").inner_text())

    def test_recent_first_download_and_download_all(self):
        email = self.email()
        self.activate(self.page, email)
        self.open_journal(self.page)
        today = date.today()
        entries = [
            {
                "iso": (today - timedelta(days=offset)).isoformat(),
                "text": f"recent-first entry {offset}",
            }
            for offset in range(35)
        ]
        self.seed(self.page, entries)
        self.assertIn("Synced 35 up", self.setup_sync(self.page))

        device_two = self.new_page()
        self.signin(device_two, email)
        self.open_journal(device_two)
        self.assertIn("30 down", self.unlock_sync(device_two))
        panel = device_two.locator("#sync-content").inner_text()
        self.assertIn("5 older entries are not downloaded", panel)
        self.assertIn("Download all entries", panel)
        self.assertEqual(len(self.local_files(device_two)), 30)

        device_two.click('[data-sync-action="sync-all"]')
        self.wait_for_sync(device_two)
        device_two.wait_for_timeout(1000)
        self.assertNotIn(
            "older entries are not downloaded",
            device_two.locator("#sync-content").inner_text(),
        )
        self.assertEqual(len(self.local_files(device_two)), 35)



if __name__ == "__main__":
    unittest.main()
