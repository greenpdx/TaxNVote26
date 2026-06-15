import { test, expect } from '@playwright/test'

// Full happy-path: open the app, set/keep an allocation, submit via the login
// dialog, confirm the receipt + auto-logout (which resets the budget), then
// verify the People's Budget aggregate updates with our submission.
test('login → submit → auto-logout resets, Results updates', async ({ page }) => {
  await page.goto('/')

  // Tree finished loading (a topic name is visible).
  await expect(page.getByText('Defense', { exact: true })).toBeVisible({ timeout: 15000 })

  // Pre-submission: read the aggregate response directly (avoids racing the
  // DOM, which briefly shows "0 submissions" while the fetch is in flight).
  const aggFirst = page.waitForResponse(r => r.url().includes('/api/aggregate'))
  await page.getByRole('button', { name: 'Results' }).click()
  const before = (await (await aggFirst).json()).submission_count as number

  // Back to Budget, click Submit — not signed in, so the dialog opens.
  await page.getByRole('button', { name: 'Budget' }).click()
  await page.getByRole('button', { name: 'Submit my Tax Dollar' }).click()

  const dialog = page.locator('.dialog')
  await expect(dialog).toBeVisible()

  const name = 'e2e_' + Math.random().toString(36).slice(2, 8)
  await dialog.getByPlaceholder('Name').fill(name)
  await dialog.getByPlaceholder('4-digit PIN').fill('1234')
  await dialog.locator('.d-login').click()

  // Receipt flash shown.
  await expect(page.getByText(/Submitted ✓/)).toBeVisible({ timeout: 10000 })

  // Auto-logout: the header widget shows the Login button again.
  await expect(page.locator('.id-widget').getByRole('button', { name: 'Login' })).toBeVisible()

  // Results: count incremented by 1 (cache invalidated on submit).
  const aggAfter = page.waitForResponse(r => r.url().includes('/api/aggregate'))
  await page.getByRole('button', { name: 'Results' }).click()
  expect((await (await aggAfter).json()).submission_count).toBe(before + 1)
})
