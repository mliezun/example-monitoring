require_relative "spec_helper"

RSpec.describe "example-monitoring", type: :feature do
  it "registers an organization, manages sites, and saves notification settings" do
    org_name = unique_name("Acme")
    username = unique_name("ops")
    password = "correct-horse-battery"

    visit "/register"
    fill_in "Organization name", with: org_name
    fill_in "Username", with: username
    fill_in "Password", with: password
    fill_in "Confirm password", with: password
    click_button "Create account"

    expect(page).to have_current_path("/sites", ignore_query: true)
    expect(page).to have_content("Monitored sites")

    click_link "Add site"
    fill_in "Display name", with: "Example HTTPS"
    fill_in "HTTPS URL", with: "https://example.com"
    fill_in "Poll interval (seconds, min 30)", with: "30"
    fill_in "OK status codes (comma-separated)", with: "200"
    fill_in "Attempts per poll cycle", with: "3"
    click_button "Save"

    expect(page).to have_content("Example HTTPS")
    expect(page).to have_content("https://example.com")

    click_link "← All sites"
    expect(page).to have_content("Example HTTPS")

    click_link "Notifications"
    choose "Slack incoming webhook"
    fill_in "Webhook URL (HTTPS)", with: "https://hooks.slack.com/services/T000/B000/XXXX"
    click_button "Save settings"

    expect(page).to have_content("Notification settings")
    expect(page).to have_content("Settings saved")
  end

  it "logs in with an existing account" do
    org_name = unique_name("LoginCo")
    username = unique_name("admin")
    password = "another-strong-password"

    visit "/register"
    fill_in "Organization name", with: org_name
    fill_in "Username", with: username
    fill_in "Password", with: password
    fill_in "Confirm password", with: password
    click_button "Create account"

    click_button "Log out"

    visit "/login"
    fill_in "Username", with: username
    fill_in "Password", with: password
    click_button "Log in"

    expect(page).to have_current_path("/sites", ignore_query: true)
    expect(page).to have_content(username)
  end
end
