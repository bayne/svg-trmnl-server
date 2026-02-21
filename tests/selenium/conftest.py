import os
import pytest
from selenium import webdriver
from selenium.webdriver.chrome.options import Options

BASE_URL = os.getenv("BASE_URL", "http://localhost:9080")
SCREENSHOT_DIR = os.getenv("SCREENSHOT_DIR", "screenshots")


def pytest_configure(config):
    os.makedirs(SCREENSHOT_DIR, exist_ok=True)


@pytest.fixture(scope="session")
def base_url():
    return BASE_URL


@pytest.fixture(scope="session")
def driver():
    options = Options()
    options.add_argument("--headless")
    options.add_argument("--no-sandbox")
    options.add_argument("--disable-dev-shm-usage")
    options.add_argument("--disable-gpu")
    options.add_argument("--window-size=1280,900")

    d = webdriver.Chrome(options=options)
    d.implicitly_wait(5)
    yield d
    d.quit()


def take_screenshot(driver, name: str) -> str:
    path = os.path.join(SCREENSHOT_DIR, f"{name}.png")
    driver.save_screenshot(path)
    return path


@pytest.hookimpl(tryfirst=True, hookwrapper=True)
def pytest_runtest_makereport(item, call):
    outcome = yield
    rep = outcome.get_result()
    if rep.when == "call" and rep.failed:
        driver = item.funcargs.get("driver")
        if driver:
            name = item.nodeid.replace("/", "_").replace("::", "_").replace(" ", "_")
            take_screenshot(driver, f"FAILED_{name}")
