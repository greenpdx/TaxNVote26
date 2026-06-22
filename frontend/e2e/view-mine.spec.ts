import { test, expect } from '@playwright/test'

// View my submission: requires re-login, loads the saved allocation into the
// tree, and (unlike Submit) does NOT auto-logout — so the user can actually
// see their data. Manual logout resets the budget back to defaults.
test('view-my-submission loads the saved data and stays signed in', async ({ page }) => {
  await page.goto('/#/app')
  await expect(page.getByText('Defense', { exact: true })).toBeVisible({ timeout: 15000 })

  const name = 'vm_' + Math.random().toString(36).slice(2, 8)
  const pin = '7777'

  // 1. Submit a fresh allocation so there's something to view later.
  await page.getByRole('button', { name: 'Submit my Tax Dollar' }).click()
  const dialog = page.locator('.dialog')
  await expect(dialog).toBeVisible()
  await dialog.getByPlaceholder('Name').fill(name)
  await dialog.getByPlaceholder('4-digit PIN').fill(pin)
  await dialog.locator('.d-login').click()
  await expect(page.getByText(/Submitted ✓/)).toBeVisible({ timeout: 10000 })
  // Submit auto-logged us out and reset the tree.
  await expect(page.locator('.id-widget').getByRole('button', { name: 'Login' })).toBeVisible()

  // 2. View my submission requires login again.
  await page.getByRole('button', { name: 'View my submission' }).click()
  await expect(dialog).toBeVisible()
  await dialog.getByPlaceholder('Name').fill(name)
  await dialog.getByPlaceholder('4-digit PIN').fill(pin)
  await dialog.locator('.d-login').click()

  // 3. Saved allocation loads and we stay signed in (so we can see it).
  await expect(page.getByText(/Loaded .* submission TD-/)).toBeVisible({ timeout: 10000 })
  await expect(page.locator('.id-widget').getByText(name)).toBeVisible()
  await expect(page.locator('.id-widget').getByRole('button', { name: 'Logout' })).toBeVisible()
})
