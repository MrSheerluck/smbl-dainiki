import os
import re
import time
import unittest
from pathlib import Path

from playwright.sync_api import sync_playwright

BASE_URL = os.environ.get("DAINIKI_URL", "http://127.0.0.1:3000")
OUTBOX = Path(
    os.environ.get(
        "DAINIKI_OUTBOX",
        Path(__file__).resolve().parents[2] / "data" / "outbox",
    )
)
PASSWORD = "correct horse battery staple"
NEW_PASSWORD = "another quiet passphrase"


class DainikiAuthE2E(unittest.TestCase):
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
            viewport={"width": 1280, "height": 1200}
        )
        self.page = self.context.new_page()
        self.page_errors = []
        self.page.on("pageerror", lambda error: self.page_errors.append(str(error)))

    def tearDown(self):
        self.context.close()
        self.assertEqual(self.page_errors, [])

    def email(self):
        return f"e2e+{time.time_ns()}@example.com"

    def goto(self, path):
        self.page.goto(f"{BASE_URL}{path}", wait_until="networkidle")

    def signup(self, email, password=PASSWORD, plan="monthly"):
        self.goto("/signup")
        self.page.fill("#signup-email", email)
        self.page.fill("#signup-password", password)
        self.page.fill("#signup-confirm", password)
        self.page.check(f'input[name="plan"][value="{plan}"]')
        self.page.click("#signup-submit")
        self.page.wait_for_load_state("networkidle")

    def signin(self, email, password=PASSWORD):
        self.goto("/signin")
        self.page.fill("#signin-email", email)
        self.page.fill("#signin-password", password)
        self.page.click("#signin-submit")
        self.page.wait_for_load_state("networkidle")

    def latest_email(self, email):
        safe_email = email.replace("+", "_")
        files = sorted(OUTBOX.glob(f"*{safe_email}*.eml"))
        self.assertTrue(files, f"no outbox email for {email} in {OUTBOX}")
        return files[-1].read_text()

    def link_from_email(self, email, kind):
        pattern = rf"https?://[^\s]+/{kind}\?token=[A-Za-z0-9_-]+"
        match = re.search(pattern, self.latest_email(email))
        self.assertIsNotNone(match, f"no {kind} link in the latest email")
        return match.group(0)

    def verify_email(self, email):
        self.page.goto(self.link_from_email(email, "verify-email"), wait_until="networkidle")
        self.assertEqual(self.page.locator("h1").inner_text(), "Email verified")

    def test_anonymous_account_redirects_to_signin(self):
        self.goto("/account")
        self.assertEqual(self.page.url, f"{BASE_URL}/signin")

    def test_signup_signout_and_signin(self):
        email = self.email()
        self.signup(email, plan="yearly")

        self.assertEqual(self.page.url.split("?")[0], f"{BASE_URL}/account")
        self.assertEqual(self.page.locator("h1").inner_text(), "Your account")
        self.assertIn(email, self.page.locator("dl").inner_text())
        self.assertIn("Yearly", self.page.locator("dl").inner_text())
        self.assertIn("Email not verified", self.page.locator("dl").inner_text())

        cookies = self.context.cookies()
        self.assertEqual(len(cookies), 1)
        session = cookies[0]
        self.assertIn(session["name"], ("dainiki_session", "__Host-dainiki_session"))
        self.assertTrue(session["httpOnly"])
        self.assertEqual(session["sameSite"], "Lax")

        self.page.click("#signout")
        self.page.wait_for_load_state("networkidle")
        self.assertEqual(self.page.url, f"{BASE_URL}/")
        self.assertEqual(self.context.cookies(), [])

        self.goto("/account")
        self.assertEqual(self.page.url, f"{BASE_URL}/signin")

        self.signin(email, password="the wrong password is long")
        self.assertEqual(self.page.url, f"{BASE_URL}/signin")
        self.assertIn("incorrect", self.page.locator('[role="alert"]').inner_text())

        self.page.fill("#signin-password", PASSWORD)
        self.page.click("#signin-submit")
        self.page.wait_for_load_state("networkidle")
        self.assertEqual(self.page.url.split("?")[0], f"{BASE_URL}/account")
        self.assertIn(email, self.page.locator("dl").inner_text())

    def test_email_verification(self):
        email = self.email()
        self.signup(email)
        self.assertIn("Email not verified", self.page.locator("dl").inner_text())

        self.verify_email(email)
        self.goto("/account")
        self.assertIn("Email verified", self.page.locator("dl").inner_text())
        self.assertEqual(self.page.locator("#resend-verification").count(), 0)

    def test_password_reset(self):
        email = self.email()
        self.signup(email)
        self.verify_email(email)
        self.goto("/account")
        self.page.click("#signout")
        self.page.wait_for_load_state("networkidle")

        self.goto("/forgot-password")
        self.page.fill("#forgot-email", email)
        self.page.click("#forgot-submit")
        self.page.wait_for_load_state("networkidle")
        self.assertIn("reset link is on its way", self.page.locator("body").inner_text())

        self.page.goto(
            self.link_from_email(email, "reset-password"), wait_until="networkidle"
        )
        self.assertEqual(self.page.locator("h1").inner_text(), "Choose a new password")
        self.page.fill("#reset-password", NEW_PASSWORD)
        self.page.fill("#reset-confirm", NEW_PASSWORD)
        self.page.click("#reset-submit")
        self.page.wait_for_load_state("networkidle")
        self.assertIn("password-reset", self.page.url)

        self.signin(email, password=PASSWORD)
        self.assertIn("incorrect", self.page.locator('[role="alert"]').inner_text())
        self.page.fill("#signin-password", NEW_PASSWORD)
        self.page.click("#signin-submit")
        self.page.wait_for_load_state("networkidle")
        self.assertEqual(self.page.url.split("?")[0], f"{BASE_URL}/account")

    def test_csrf_required_for_signout(self):
        email = self.email()
        self.signup(email)

        status = self.page.evaluate(
            """async () => {
                const response = await fetch("/signout", {
                    method: "POST",
                    headers: {"Content-Type": "application/x-www-form-urlencoded"},
                    body: "csrf_token=wrong",
                });
                return response.status;
            }"""
        )
        self.assertEqual(status, 403)
        self.assertEqual(self.page.url.split("?")[0], f"{BASE_URL}/account")

    def test_account_password_change_and_delete(self):
        email = self.email()
        self.signup(email)
        self.verify_email(email)
        self.goto("/account")

        self.page.fill("#current-password", PASSWORD)
        self.page.fill("#new-password", NEW_PASSWORD)
        self.page.fill("#new-password-confirm", NEW_PASSWORD)
        self.page.click("#change-password")
        self.page.wait_for_load_state("networkidle")
        self.assertIn("password-changed", self.page.url)

        self.page.click("#signout")
        self.page.wait_for_load_state("networkidle")
        self.signin(email, password=NEW_PASSWORD)
        self.assertEqual(self.page.url.split("?")[0], f"{BASE_URL}/account")

        self.page.locator("summary").click()
        self.page.fill("#delete-password", NEW_PASSWORD)
        self.page.fill("#delete-confirmation", "DELETE")
        self.page.click("#delete-account")
        self.page.wait_for_load_state("networkidle")
        self.assertEqual(self.page.url, f"{BASE_URL}/")

        self.goto("/account")
        self.assertEqual(self.page.url, f"{BASE_URL}/signin")

    def test_password_mismatch_is_rejected(self):
        email = self.email()
        self.goto("/signup")
        self.page.fill("#signup-email", email)
        self.page.fill("#signup-password", PASSWORD)
        self.page.fill("#signup-confirm", "a different long password")
        self.page.click("#signup-submit")
        self.page.wait_for_load_state("networkidle")

        self.assertEqual(self.page.url, f"{BASE_URL}/signup")
        self.assertIn(
            "do not match", self.page.locator('[role="alert"]').inner_text()
        )
        self.assertEqual(self.context.cookies(), [])

    def test_duplicate_email_is_rejected(self):
        email = self.email()
        self.signup(email)
        self.assertIn("Your account", self.page.locator("h1").inner_text())

        self.page.click("#signout")
        self.page.wait_for_load_state("networkidle")
        self.signup(email)

        self.assertEqual(self.page.url, f"{BASE_URL}/signup")
        self.assertIn("already exists", self.page.locator('[role="alert"]').inner_text())


if __name__ == "__main__":
    unittest.main()
