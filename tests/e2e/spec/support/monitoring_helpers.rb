# Shared helpers for readable integration tests.
#
# Design goal: specs read like short stories —
#   register → add site → wait until poller writes a row → assert on facts
module MonitoringHelpers
  MOCK_HTTPS = ENV.fetch("MOCK_HTTPS_BASE", "https://mock-https:8443").freeze

  def mock_status_url(code)
    "#{MOCK_HTTPS}/status/#{code}"
  end

  def monitoring_db
    @monitoring_db ||= begin
      db = SQLite3::Database.new(ENV.fetch("DATABASE_PATH", "data/monitoring.db"))
      db.results_as_hash = true
      db
    end
  end

  def register_fresh_account(org_name: unique_name("PollerOrg"))
    @current_username = unique_name("poller")
    password = "correct-horse-battery"

    visit "/register"
    fill_in "Organization name", with: org_name
    fill_in "Username", with: @current_username
    fill_in "Password", with: password
    fill_in "Confirm password", with: password
    click_button "Create account"

    expect(page).to have_current_path("/sites", ignore_query: true)
    @current_username
  end

  def add_monitored_site(name:, url:, ok_codes: "200", retries: 3, interval: 30)
    visit "/sites/new"
    fill_in "Display name", with: name
    fill_in "HTTPS URL", with: url
    fill_in "Poll interval (seconds, min 30)", with: interval.to_s
    fill_in "OK status codes (comma-separated)", with: ok_codes
    fill_in "Attempts per poll cycle", with: retries.to_s
    click_button "Save"
    expect(page).to have_content(name)
    name
  end

  # Wait until a block returns a truthy value, with a human-readable reason.
  def wait_until(reason, timeout: 45, interval: 0.5)
    deadline = Time.now + timeout
    loop do
      value = yield
      return value if value

      raise "Timed out after #{timeout}s waiting for: #{reason}" if Time.now >= deadline

      sleep interval
    end
  end

  def force_site_due_for_poll(site_name)
    monitoring_db.execute(<<~SQL, [site_name])
      UPDATE sites
      SET next_poll_at = datetime('now')
      WHERE name = ?
    SQL
  end

  def set_site_url(site_name, url)
    monitoring_db.execute(<<~SQL, [url, site_name])
      UPDATE sites
      SET url = ?
      WHERE name = ?
    SQL
    force_site_due_for_poll(site_name)
  end

  def poll_row(site_name)
    monitoring_db.execute(<<~SQL, [site_name]).first
      SELECT
        s.name AS site_name,
        s.current_status,
        s.url,
        pr.status AS poll_status,
        pr.http_status_code,
        pr.attempts_used,
        pr.error_message,
        pr.checked_at
      FROM sites s
      LEFT JOIN poll_results pr ON pr.site_id = s.id
      WHERE s.name = ?
      ORDER BY pr.checked_at DESC
      LIMIT 1
    SQL
  end

  def poll_count_for(site_name)
    monitoring_db.get_first_value(<<~SQL, [site_name]).to_i
      SELECT COUNT(*)
      FROM poll_results pr
      JOIN sites s ON s.id = pr.site_id
      WHERE s.name = ?
    SQL
  end
end

RSpec.configure do |config|
  config.include MonitoringHelpers, type: :integration
  config.include MonitoringHelpers, type: :feature
end
