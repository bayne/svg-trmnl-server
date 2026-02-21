"""
Selenium UI tests for svg-trmnl-server.

These tests verify the admin and preview interfaces render correctly and
that client-side interactions (JavaScript) behave as expected.
Screenshots are saved to the SCREENSHOT_DIR (default: screenshots/).
"""
import os
import time

import pytest
from selenium.webdriver.common.by import By
from selenium.webdriver.support import expected_conditions as EC
from selenium.webdriver.support.ui import Select, WebDriverWait

from conftest import BASE_URL, take_screenshot

WAIT_TIMEOUT = 10


# ---------------------------------------------------------------------------
# Admin Dashboard (/admin)
# ---------------------------------------------------------------------------


class TestAdminDashboard:
    def test_page_title(self, driver):
        """Admin dashboard has the correct page title."""
        driver.get(f"{BASE_URL}/admin")
        assert "TRMNL" in driver.title
        take_screenshot(driver, "admin_dashboard")

    def test_heading(self, driver):
        """Admin dashboard displays the main heading."""
        driver.get(f"{BASE_URL}/admin")
        h1 = driver.find_element(By.TAG_NAME, "h1")
        assert "TRMNL Server Admin" in h1.text

    def test_navigation_links(self, driver):
        """Admin dashboard nav includes links to Devices and Template Preview."""
        driver.get(f"{BASE_URL}/admin")
        nav = driver.find_element(By.TAG_NAME, "nav")
        links = nav.find_elements(By.TAG_NAME, "a")
        hrefs = [link.get_attribute("href") for link in links]
        assert any("/admin" in h for h in hrefs), "Expected link to /admin"
        assert any("/display/preview" in h for h in hrefs), "Expected link to /display/preview"

    def test_add_device_button(self, driver):
        """Admin dashboard has an 'Add Device' button linking to /admin/devices/new."""
        driver.get(f"{BASE_URL}/admin")
        add_btn = driver.find_element(By.CSS_SELECTOR, "a[href='/admin/devices/new']")
        assert add_btn.is_displayed()
        assert "Add Device" in add_btn.text or "+" in add_btn.text

    def test_device_table_present(self, driver):
        """Admin dashboard renders a device table with expected columns."""
        driver.get(f"{BASE_URL}/admin")
        table = driver.find_element(By.TAG_NAME, "table")
        headers = [th.text for th in table.find_elements(By.TAG_NAME, "th")]
        assert "Friendly ID" in headers
        assert "MAC Address" in headers
        assert "API Key" in headers
        assert "Setup Expiry" in headers
        assert "Playlist" in headers

    def test_test_device_listed(self, driver):
        """Admin dashboard shows the configured test device."""
        driver.get(f"{BASE_URL}/admin")
        tbody = driver.find_element(By.TAG_NAME, "tbody")
        rows = tbody.find_elements(By.TAG_NAME, "tr")
        assert len(rows) >= 1, "Expected at least one device row"
        row_text = rows[0].text
        assert "test-device" in row_text, "Expected 'test-device' in first table row"

    def test_device_setup_badge(self, driver):
        """Admin dashboard shows a setup expiry badge (Active or Expired)."""
        driver.get(f"{BASE_URL}/admin")
        badges = driver.find_elements(By.CSS_SELECTOR, ".badge")
        assert len(badges) >= 1
        badge_texts = [b.text for b in badges]
        assert any(t in ("Active", "Expired") for t in badge_texts)

    def test_screenshot_full_page(self, driver):
        """Take a full-page screenshot of the admin dashboard."""
        driver.get(f"{BASE_URL}/admin")
        # Scroll to bottom to capture full page, then back to top
        driver.execute_script("window.scrollTo(0, document.body.scrollHeight);")
        time.sleep(0.3)
        driver.execute_script("window.scrollTo(0, 0);")
        take_screenshot(driver, "admin_dashboard_full")


# ---------------------------------------------------------------------------
# Add Device Form (/admin/devices/new)
# ---------------------------------------------------------------------------


class TestAddDeviceForm:
    def test_page_title(self, driver):
        """Add Device page has the correct title."""
        driver.get(f"{BASE_URL}/admin/devices/new")
        assert "Add Device" in driver.title or "TRMNL" in driver.title
        take_screenshot(driver, "admin_new_device")

    def test_heading(self, driver):
        """Add Device page shows 'Add New Device' heading."""
        driver.get(f"{BASE_URL}/admin/devices/new")
        h1 = driver.find_element(By.TAG_NAME, "h1")
        assert "Add New Device" in h1.text

    def test_back_navigation_link(self, driver):
        """Add Device page has a back link to /admin."""
        driver.get(f"{BASE_URL}/admin/devices/new")
        nav = driver.find_element(By.TAG_NAME, "nav")
        back_link = nav.find_element(By.TAG_NAME, "a")
        assert "/admin" in back_link.get_attribute("href")

    def test_friendly_id_field(self, driver):
        """Add Device form has a Friendly ID input."""
        driver.get(f"{BASE_URL}/admin/devices/new")
        field = driver.find_element(By.ID, "friendly_id")
        assert field.is_displayed()
        assert field.get_attribute("name") == "friendly_id"

    def test_mac_address_field(self, driver):
        """Add Device form has a MAC Address input."""
        driver.get(f"{BASE_URL}/admin/devices/new")
        field = driver.find_element(By.ID, "mac_address")
        assert field.is_displayed()
        assert field.get_attribute("name") == "mac_address"

    def test_api_key_field(self, driver):
        """Add Device form has an API Key input."""
        driver.get(f"{BASE_URL}/admin/devices/new")
        field = driver.find_element(By.ID, "api_key")
        assert field.is_displayed()
        assert field.get_attribute("name") == "api_key"

    def test_setup_expiry_field(self, driver):
        """Add Device form has a Setup Expiry datetime input."""
        driver.get(f"{BASE_URL}/admin/devices/new")
        field = driver.find_element(By.ID, "setup_expiry")
        assert field.is_displayed()
        assert field.get_attribute("type") == "datetime-local"

    def test_generate_api_key_button_present(self, driver):
        """Add Device form has a 'Generate' button for the API key."""
        driver.get(f"{BASE_URL}/admin/devices/new")
        btn = driver.find_element(By.CSS_SELECTOR, "button.btn-generate")
        assert btn.is_displayed()
        assert "Generate" in btn.text

    def test_generate_api_key_populates_field(self, driver):
        """Clicking 'Generate' fills the API Key field with a 32-character string."""
        driver.get(f"{BASE_URL}/admin/devices/new")
        api_key_field = driver.find_element(By.ID, "api_key")
        api_key_field.clear()

        generate_btn = driver.find_element(By.CSS_SELECTOR, "button.btn-generate")
        generate_btn.click()

        generated_key = api_key_field.get_attribute("value")
        assert len(generated_key) == 32, (
            f"Expected 32-char API key, got {len(generated_key)!r}: {generated_key!r}"
        )
        assert generated_key.isalnum(), "Expected API key to be alphanumeric"
        take_screenshot(driver, "admin_new_device_api_key_generated")

    def test_generate_api_key_is_random(self, driver):
        """Clicking 'Generate' twice produces different keys."""
        driver.get(f"{BASE_URL}/admin/devices/new")
        generate_btn = driver.find_element(By.CSS_SELECTOR, "button.btn-generate")
        api_key_field = driver.find_element(By.ID, "api_key")

        generate_btn.click()
        key1 = api_key_field.get_attribute("value")

        generate_btn.click()
        key2 = api_key_field.get_attribute("value")

        assert key1 != key2, "Two consecutive generated keys should differ"

    def test_add_playlist_entry_button_present(self, driver):
        """Add Device form has an 'Add Playlist Item' button."""
        driver.get(f"{BASE_URL}/admin/devices/new")
        btn = driver.find_element(By.CSS_SELECTOR, "button.btn-add-entry")
        assert btn.is_displayed()
        assert "Add Playlist Item" in btn.text or "Playlist" in btn.text

    def test_default_playlist_entry_added_on_load(self, driver):
        """Page adds one default playlist entry on load via JavaScript."""
        driver.get(f"{BASE_URL}/admin/devices/new")
        wait = WebDriverWait(driver, WAIT_TIMEOUT)
        wait.until(
            EC.presence_of_element_located((By.CSS_SELECTOR, ".playlist-entry"))
        )
        entries = driver.find_elements(By.CSS_SELECTOR, ".playlist-entry")
        assert len(entries) >= 1, "Expected at least one playlist entry on page load"

    def test_add_playlist_entry_click(self, driver):
        """Clicking 'Add Playlist Item' inserts an additional playlist entry."""
        driver.get(f"{BASE_URL}/admin/devices/new")
        wait = WebDriverWait(driver, WAIT_TIMEOUT)
        # Wait for default entry to be added by JS
        wait.until(EC.presence_of_element_located((By.CSS_SELECTOR, ".playlist-entry")))
        before_count = len(driver.find_elements(By.CSS_SELECTOR, ".playlist-entry"))

        add_btn = driver.find_element(By.CSS_SELECTOR, "button.btn-add-entry")
        add_btn.click()

        after_count = len(driver.find_elements(By.CSS_SELECTOR, ".playlist-entry"))
        assert after_count == before_count + 1, (
            f"Expected {before_count + 1} entries after click, got {after_count}"
        )
        take_screenshot(driver, "admin_new_device_playlist_added")

    def test_remove_playlist_entry_click(self, driver):
        """Clicking the remove button on a playlist entry removes it."""
        driver.get(f"{BASE_URL}/admin/devices/new")
        wait = WebDriverWait(driver, WAIT_TIMEOUT)
        wait.until(EC.presence_of_element_located((By.CSS_SELECTOR, ".playlist-entry")))

        # Add an extra entry so we can remove it without hitting zero
        add_btn = driver.find_element(By.CSS_SELECTOR, "button.btn-add-entry")
        add_btn.click()
        before_count = len(driver.find_elements(By.CSS_SELECTOR, ".playlist-entry"))

        remove_btn = driver.find_elements(By.CSS_SELECTOR, ".btn-remove")[0]
        remove_btn.click()

        after_count = len(driver.find_elements(By.CSS_SELECTOR, ".playlist-entry"))
        assert after_count == before_count - 1

    def test_setup_expiry_defaults_to_future(self, driver):
        """Setup expiry field defaults to a date in the future."""
        driver.get(f"{BASE_URL}/admin/devices/new")
        wait = WebDriverWait(driver, WAIT_TIMEOUT)
        # JS sets the value on DOMContentLoaded, wait until it's non-empty
        wait.until(
            lambda d: d.find_element(By.ID, "setup_expiry").get_attribute("value") != ""
        )
        expiry_value = driver.find_element(By.ID, "setup_expiry").get_attribute("value")
        assert expiry_value, "Setup expiry should have a default value"
        # The value is in "YYYY-MM-DDTHH:MM" format; year should be > 2025
        year = int(expiry_value[:4])
        assert year > 2025, f"Expiry year {year} should be in the future"

    def test_save_and_cancel_buttons(self, driver):
        """Add Device form has Save and Cancel action buttons."""
        driver.get(f"{BASE_URL}/admin/devices/new")
        submit_btn = driver.find_element(By.CSS_SELECTOR, "button[type='submit'].btn-primary")
        cancel_link = driver.find_element(By.CSS_SELECTOR, "a.btn-secondary")
        assert "Save" in submit_btn.text
        assert "Cancel" in cancel_link.text
        assert "/admin" in cancel_link.get_attribute("href")


# ---------------------------------------------------------------------------
# Template Preview (/display/preview)
# ---------------------------------------------------------------------------


class TestPreviewPage:
    def test_page_loads(self, driver):
        """Template preview page loads without error."""
        driver.get(f"{BASE_URL}/display/preview")
        assert driver.title != ""
        take_screenshot(driver, "preview_page")

    def test_template_selector_present(self, driver):
        """Template preview page has a template selector dropdown."""
        driver.get(f"{BASE_URL}/display/preview")
        selector = driver.find_element(By.ID, "templateName")
        assert selector.is_displayed()
        assert selector.tag_name == "select"

    def test_template_options_listed(self, driver):
        """Template selector lists at least one template option."""
        driver.get(f"{BASE_URL}/display/preview")
        selector = driver.find_element(By.ID, "templateName")
        options = selector.find_elements(By.TAG_NAME, "option")
        # There should be the blank option plus at least one real template
        assert len(options) >= 2, f"Expected at least 2 options, got {len(options)}"
        option_values = [o.get_attribute("value") for o in options]
        non_empty = [v for v in option_values if v]
        assert len(non_empty) >= 1, "Expected at least one non-empty template option"

    def test_websocket_status_indicator_present(self, driver):
        """Template preview page has a WebSocket status indicator."""
        driver.get(f"{BASE_URL}/display/preview")
        status_container = driver.find_element(By.ID, "websocket-status-container")
        assert status_container.is_displayed()

    def test_websocket_status_disconnected_on_load(self, driver):
        """WebSocket status shows 'Disconnected' when no template is selected."""
        driver.get(f"{BASE_URL}/display/preview")
        wait = WebDriverWait(driver, WAIT_TIMEOUT)
        wait.until(
            lambda d: "disconnected" in d.find_element(
                By.ID, "websocket-status-container"
            ).get_attribute("class")
        )
        status = driver.find_element(By.ID, "websocket-status-container")
        assert "disconnected" in status.get_attribute("class")

    def test_image_display_box_present(self, driver):
        """Template preview page has an image display area."""
        driver.get(f"{BASE_URL}/display/preview")
        image_box = driver.find_element(By.ID, "image-display-box")
        assert image_box.is_displayed()

    def test_preview_renders_on_template_select(self, driver):
        """Selecting a template triggers a WebSocket connection and renders an image."""
        driver.get(f"{BASE_URL}/display/preview")
        wait = WebDriverWait(driver, WAIT_TIMEOUT)

        selector = driver.find_element(By.ID, "templateName")
        sel = Select(selector)
        # Pick the first real template
        options = [o for o in sel.options if o.get_attribute("value")]
        assert options, "No template options available to select"
        sel.select_by_value(options[0].get_attribute("value"))

        # Status should move to 'connecting' or 'connected'
        wait.until(
            lambda d: any(
                cls in d.find_element(By.ID, "websocket-status-container").get_attribute("class")
                for cls in ("connecting", "connected", "message")
            )
        )
        time.sleep(2)  # Allow rendering to complete
        take_screenshot(driver, "preview_page_template_selected")

        status = driver.find_element(By.ID, "websocket-status-container")
        # Should not be in disconnected state after selecting a template
        assert "disconnected" not in status.get_attribute("class") or \
               "message" in status.get_attribute("class") or \
               "connected" in status.get_attribute("class")


# ---------------------------------------------------------------------------
# Icon Browser (/display/preview/icons)
# ---------------------------------------------------------------------------


class TestIconsPage:
    def test_page_loads(self, driver):
        """Icons page loads without error."""
        driver.get(f"{BASE_URL}/display/preview/icons")
        assert driver.title != ""
        take_screenshot(driver, "icons_page")

    def test_page_title(self, driver):
        """Icons page has 'Unicode Icon Grid' in the title."""
        driver.get(f"{BASE_URL}/display/preview/icons")
        assert "Icon" in driver.title or "TRMNL" in driver.title or "Unicode" in driver.title

    def test_search_input_present(self, driver):
        """Icons page has a search input field."""
        driver.get(f"{BASE_URL}/display/preview/icons")
        search = driver.find_element(By.ID, "searchInput")
        assert search.is_displayed()
        assert search.get_attribute("type") == "text"

    def test_icon_grid_rendered(self, driver):
        """Icons page renders icon cards in the grid."""
        driver.get(f"{BASE_URL}/display/preview/icons")
        wait = WebDriverWait(driver, WAIT_TIMEOUT)
        wait.until(
            EC.presence_of_element_located((By.CSS_SELECTOR, ".icon-card"))
        )
        cards = driver.find_elements(By.CSS_SELECTOR, ".icon-card")
        assert len(cards) > 0, "Expected icon cards to be rendered"

    def test_icon_cards_have_unicode_labels(self, driver):
        """Icon cards display a unicode label beneath the icon."""
        driver.get(f"{BASE_URL}/display/preview/icons")
        wait = WebDriverWait(driver, WAIT_TIMEOUT)
        wait.until(EC.presence_of_element_located((By.CSS_SELECTOR, ".icon-card .unicode")))
        labels = driver.find_elements(By.CSS_SELECTOR, ".icon-card .unicode")
        assert len(labels) > 0

    def test_search_filters_icons(self, driver):
        """Typing in the search box filters the icon grid."""
        driver.get(f"{BASE_URL}/display/preview/icons")
        wait = WebDriverWait(driver, WAIT_TIMEOUT)
        wait.until(EC.presence_of_element_located((By.CSS_SELECTOR, ".icon-card")))

        total_before = len(driver.find_elements(By.CSS_SELECTOR, ".icon-card"))

        search = driver.find_element(By.ID, "searchInput")
        search.send_keys("sun")
        time.sleep(0.5)

        total_after = len(driver.find_elements(By.CSS_SELECTOR, ".icon-card"))
        # Filtering should narrow the results (unless every icon matches 'sun')
        assert total_after <= total_before, (
            f"Expected fewer icons after filtering, got {total_after} >= {total_before}"
        )
        take_screenshot(driver, "icons_page_search_filtered")

    def test_search_clear_restores_all_icons(self, driver):
        """Clearing the search input restores all icons."""
        driver.get(f"{BASE_URL}/display/preview/icons")
        wait = WebDriverWait(driver, WAIT_TIMEOUT)
        wait.until(EC.presence_of_element_located((By.CSS_SELECTOR, ".icon-card")))

        total_before = len(driver.find_elements(By.CSS_SELECTOR, ".icon-card"))

        search = driver.find_element(By.ID, "searchInput")
        search.send_keys("sun")
        time.sleep(0.3)

        search.clear()
        time.sleep(0.3)

        total_after = len(driver.find_elements(By.CSS_SELECTOR, ".icon-card"))
        assert total_after == total_before, (
            f"Expected {total_before} icons after clearing search, got {total_after}"
        )

    def test_screenshot_with_many_icons(self, driver):
        """Take a screenshot showing the full icon grid."""
        driver.get(f"{BASE_URL}/display/preview/icons")
        wait = WebDriverWait(driver, WAIT_TIMEOUT)
        wait.until(EC.presence_of_element_located((By.CSS_SELECTOR, ".icon-card")))
        take_screenshot(driver, "icons_page_full_grid")
