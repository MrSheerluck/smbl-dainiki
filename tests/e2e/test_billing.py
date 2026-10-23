import base64
import hashlib
import hmac
import json
import os
import re
import time
import unittest
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

from playwright.sync_api import sync_playwright

BASE_URL = os.environ.get("DAINIKI_URL", "http://127.0.0.1:3000")
OUTBOX = Path(
    os.environ.get(
        "DAINIKI_OUTBOX",
        Path(__file__).resolve().parents[2] / "data" / "outbox",
    )
)
WEBHOOK_SECRET = os.environ.get("DAINIKI_WEBHOOK_SECRET")
PASSWORD = "correct horse battery staple"


class DainikiBillingE2E(unittest.TestCase):
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
        return f"billing+{time.time_ns()}@example.com"

    def goto(self, path):
        self.page.goto(f"{BASE_URL}{path}", wait_until="domcontentloaded")

    def signup(self, email):
        self.goto("/signup")
        self.page.fill("#signup-email", email)
        self.page.fill("#signup-password", PASSWORD)
        self.page.fill("#signup-confirm", PASSWORD)
        self.page.click("#signup-submit")
        self.page.wait_for_load_state("networkidle")

    def verify(self, email):
        safe = email.replace("+", "_")
        files = sorted(OUTBOX.glob(f"*{safe}*.eml"))
        self.assertTrue(files, f"no verification email for {email}")
        match = re.search(
            r"https?://[^\s]+/verify-email\?token=[A-Za-z0-9_-]+",
            files[-1].read_text(),
        )
        self.assertIsNotNone(match)
        self.page.goto(match.group(0), wait_until="networkidle")
        self.assertEqual(self.page.locator("h1").inner_text(), "Email verified")

    def activate_mock(self, email, plan="monthly"):
        self.signup(email)
        self.verify(email)
        self.goto("/billing")
        if self.page.locator("text=Local test billing is on.").count() == 0:
            self.skipTest("billing is not running in mock mode")
        self.page.click(f'button:has-text("Subscribe {plan}")')
        self.page.wait_for_load_state("networkidle")

    def test_billing_requires_signin(self):
        self.goto("/billing")
        self.assertEqual(self.page.url, f"{BASE_URL}/signup")

    def test_mock_checkout_activates_plan(self):
        email = self.email()
        self.activate_mock(email, plan="yearly")

        self.assertIn("mock-active", self.page.url)
        body = self.page.locator("body").inner_text()
        self.assertIn("Current plan", body)
        self.assertIn("Yearly · $20 / year", body)
        self.assertIn("ACTIVE", body.upper())

        self.goto("/account")
        account = self.page.locator("body").inner_text()
        self.assertIn("Billing", account)
        self.assertIn("Yearly · $20 / year · Active", account)

    def test_unverified_account_cannot_subscribe(self):
        email = self.email()
        self.signup(email)
        self.goto("/billing")
        self.assertIn("Verify your email", self.page.locator("body").inner_text())
        self.assertTrue(
            self.page.locator('button:has-text("Subscribe monthly")').is_disabled()
        )

    def test_checkout_requires_csrf(self):
        email = self.email()
        self.signup(email)
        self.verify(email)

        status = self.page.evaluate(
            """async () => {
                const body = new URLSearchParams({csrf_token: "wrong", plan: "monthly"});
                const response = await fetch("/billing/checkout", {
                    method: "POST",
                    headers: {"Content-Type": "application/x-www-form-urlencoded"},
                    body,
                });
                return response.status;
            }"""
        )
        self.assertEqual(status, 403)

    @unittest.skipUnless(WEBHOOK_SECRET, "DAINIKI_WEBHOOK_SECRET is not set")
    def test_signed_subscription_webhook_updates_billing(self):
        email = self.email()
        self.activate_mock(email, plan="monthly")

        event_id = f"evt_billing_{time.time_ns()}"
        timestamp = int(time.time())
        event_time = (
            datetime.now(timezone.utc)
            .replace(microsecond=0)
            .isoformat()
            .replace("+00:00", "Z")
        )
        payload = {
            "business_id": "bus_test",
            "type": "subscription.active",
            "timestamp": event_time,
            "data": {
                "payload_type": "Subscription",
                "subscription_id": f"sub_test_{time.time_ns()}",
                "status": "active",
                "product_id": "pdt_test_yearly",
                "next_billing_date": "2027-10-23T00:00:00Z",
                "cancel_at_next_billing_date": False,
                "currency": "USD",
                "recurring_pre_tax_amount": 2000,
                "customer": {
                    "customer_id": f"cus_test_{time.time_ns()}",
                    "email": email,
                },
                "metadata": {"plan": "yearly"},
            },
        }
        body = json.dumps(payload, separators=(",", ":")).encode()
        signature = self.sign(event_id, timestamp, body)

        status = self.post_webhook(
            event_id, timestamp, signature, body
        )
        self.assertEqual(status, 200)
        self.assertEqual(
            self.post_webhook(event_id, timestamp, signature, body), 200
        )

        self.goto("/billing")
        self.assertIn("Yearly · $20 / year", self.page.locator("body").inner_text())

        with self.assertRaises(urllib.error.HTTPError) as error:
            self.post_webhook(event_id, timestamp, "v1,bad", body)
        self.assertEqual(error.exception.code, 401)

    def sign(self, event_id, timestamp, body):
        key = base64.b64decode(
            WEBHOOK_SECRET.removeprefix("whsec_")
        )
        signed = f"{event_id}.{timestamp}.".encode() + body
        digest = hmac.new(key, signed, hashlib.sha256).digest()
        return f"v1,{base64.b64encode(digest).decode()}"

    def post_webhook(self, event_id, timestamp, signature, body):
        request = urllib.request.Request(
            f"{BASE_URL}/api/billing/webhook",
            data=body,
            method="POST",
            headers={
                "Content-Type": "application/json",
                "webhook-id": event_id,
                "webhook-timestamp": str(timestamp),
                "webhook-signature": signature,
            },
        )
        with urllib.request.urlopen(request) as response:
            return response.status


if __name__ == "__main__":
    unittest.main()
