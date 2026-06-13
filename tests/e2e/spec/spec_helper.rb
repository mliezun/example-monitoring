require "capybara/rspec"
require "capybara/cuprite"
require "sqlite3"

Dir[File.join(__dir__, "support", "**", "*.rb")].sort.each { |file| require file }

APP_URL = ENV.fetch("APP_URL", "http://localhost:8000")

Capybara.app_host = APP_URL
Capybara.default_max_wait_time = 10
Capybara.default_driver = :cuprite
Capybara.javascript_driver = :cuprite

Capybara.register_driver(:cuprite) do |app|
  Capybara::Cuprite::Driver.new(
    app,
    window_size: [1400, 900],
    browser_options: {
      "no-sandbox": nil,
      "disable-dev-shm-usage": nil,
    },
    headless: true,
    process_timeout: 30,
    timeout: 10,
    browser_path: ENV.fetch("CHROME_PATH", "/usr/bin/chromium"),
  )
end

RSpec.configure do |config|
  config.expect_with :rspec do |expectations|
    expectations.include_chain_clauses_in_custom_matcher_descriptions = true
  end

  config.mock_with :rspec do |mocks|
    mocks.verify_partial_doubles = true
  end

  config.shared_context_metadata_behavior = :apply_to_host_groups
  config.filter_run_when_matching :focus
  config.example_status_persistence_file_path = ".rspec_status"
  config.disable_monkey_patching!
  config.order = :random
  Kernel.srand config.seed

  config.include Capybara::DSL, type: :feature
  config.include Capybara::DSL, type: :integration

  config.before(type: :feature) do
    Capybara.current_driver = :cuprite
    Capybara.app_host = APP_URL
  end

  config.before(type: :integration) do
    Capybara.current_driver = :cuprite
    Capybara.app_host = APP_URL
  end
end

def unique_name(prefix)
  "#{prefix}-#{Time.now.to_i}-#{rand(1000..9999)}"
end

def fill_csrf_if_needed
  return unless page.has_css?('input[name="_csrf"]', visible: :all, wait: 0.5)

  token = find('input[name="_csrf"]', visible: :all).value
  page.execute_script(<<~JS)
    document.querySelectorAll('input[name="_csrf"]').forEach((input) => {
      input.value = #{token.to_json};
    });
  JS
end
