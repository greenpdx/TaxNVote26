import { test, expect } from '@playwright/test'

// Template registry: an identified person can save the current budget as a
// template with a name + entity + description; anyone can list and load it.
test('save current as template, list it, and load it back', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByText('Defense', { exact: true })).toBeVisible({ timeout: 15000 })

  const suffix = Math.random().toString(36).slice(2, 8)
  const tplName = `QA Template ${suffix}`
  const entity = `QA Org ${suffix}`

  // 1. Save as template → must log in first (button is gated by requireLogin).
  await page.getByRole('button', { name: 'Save as template' }).click()
  const dialog = page.locator('.dialog')
  await expect(dialog).toBeVisible()
  await dialog.getByPlaceholder('Name').fill('tpl_' + suffix)
  await dialog.getByPlaceholder('4-digit PIN').fill('8888')
  await dialog.locator('.d-login').click()
  await expect(dialog).toBeHidden()

  // The save form is now revealed.
  await page.getByPlaceholder('Template name (≥3)').fill(tplName)
  await page.getByPlaceholder('Entity (org) name').fill(entity)
  await page.getByPlaceholder('Description (optional)').fill('e2e smoke')
  await page.getByRole('button', { name: 'Save', exact: true }).click()

  await expect(page.getByText(/Saved template TPL-/)).toBeVisible({ timeout: 10000 })

  // 2. Logout (resets the budget — doesn't affect the registry).
  await page.locator('.id-widget').getByRole('button', { name: 'Logout' }).click()
  await expect(page.locator('.id-widget').getByRole('button', { name: 'Login' })).toBeVisible()

  // 3. Templates tab: our entry is listed with the entity name.
  await page.getByRole('button', { name: 'Templates' }).click()
  const row = page.locator('.titem').filter({ hasText: tplName })
  await expect(row).toBeVisible({ timeout: 10000 })
  await expect(row).toContainText(entity)

  // 4. Load: applies the template and switches back to the Budget view
  // (TemplatesView emits 'loaded' which App reacts to). The Submit button
  // only exists on the Budget view, so its presence proves the switch.
  await row.getByRole('button', { name: /Load/ }).click()
  await expect(page.getByRole('button', { name: 'Submit my Tax Dollar' })).toBeVisible({ timeout: 10000 })
  await expect(page.getByText('Defense', { exact: true })).toBeVisible()
})
