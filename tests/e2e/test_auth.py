import os
import time
import unittest

from playwright.sync_api import sync_playwright

BASE_URL = os.environ.get("DAINIKI_URL", "http://127.0.0.1:3000")
PASSWORD = "correct horse battery staple"


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
        self.context = self.browser.new_context(viewport={"width": 1280, "height": 1000})
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

    def test_anonymous_account_redirects_to_signin(self):
        self.goto("/account")
        self.assertEqual(self.page.url, f"{BASE_URL}/signin")
        self.assertEqual(self.page.locator("h1").inner_text(), "Sign in")

    def test_signup_signout_and_signin(self):
        email = self.email()
        self.signup(email, plan="yearly")

        self.assertEqual(self.page.url, f"{BASE_URL}/account")
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
        self.assertEqual(self.page.url, f"{BASE_URL}/account")
        self.assertIn(email, self.page.locator("dl").inner_text())

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
