require_relative "spec_helper"

# Background poller integration tests.
#
# These specs talk to the real SQLite database and rely on the running Node
# poller service. httpbin.org simulates HTTP success and failure — no custom
# mock server required.
RSpec.describe "Background poller", type: :integration do
  before do
    register_fresh_account
  end

  it "marks a site UP when httpbin returns 200" do
    site_name = add_monitored_site(
      name: "Healthy httpbin",
      url: "https://httpbin.org/status/200",
      retries: 1,
    )

    result = wait_until("poller records an UP result for #{site_name}") do
      row = poll_row(site_name)
      row if row && row["poll_status"] == "up"
    end

    expect(result["poll_status"]).to eq("up")
    expect(result["http_status_code"]).to eq(200)
    expect(result["attempts_used"]).to eq(1)
    expect(result["current_status"]).to eq("up")
  end

  it "marks a site DOWN after all retry attempts fail" do
    site_name = add_monitored_site(
      name: "Broken httpbin",
      url: "https://httpbin.org/status/503",
      ok_codes: "200",
      retries: 3,
    )

    result = wait_until("poller records a DOWN result after 3 attempts") do
      row = poll_row(site_name)
      row if row && row["poll_status"] == "down" && row["attempts_used"] == 3
    end

    expect(result["poll_status"]).to eq("down")
    expect(result["attempts_used"]).to eq(3)
    expect(result["current_status"]).to eq("down")
    # httpbin may answer 503 or fail entirely from CI runners; both are valid DOWN signals
    expect([503, nil]).to include(result["http_status_code"])
  end

  it "detects a status change when the endpoint starts failing" do
    site_name = add_monitored_site(
      name: "Flaky httpbin",
      url: "https://httpbin.org/status/200",
      retries: 1,
    )

    wait_until("initial UP poll") do
      row = poll_row(site_name)
      row if row&.dig("poll_status") == "up"
    end

    set_site_url(site_name, "https://httpbin.org/status/500")

    result = wait_until("poller records DOWN after URL starts returning 500") do
      row = poll_row(site_name)
      row if row && row["poll_status"] == "down" && poll_count_for(site_name) >= 2
    end

    expect(result["current_status"]).to eq("down")
    expect([500, nil]).to include(result["http_status_code"])
  end
end
