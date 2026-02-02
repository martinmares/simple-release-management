#!/bin/zsh

# Web UI Test Script
# Ověří že všechny statické soubory a hlavní stránka fungují

echo "🧪 Testing Simple Release Management Web UI..."
echo ""

BASE_URL="http://127.0.0.1:3000"

# Test health endpoint
echo "✓ Testing /health..."
curl -sf "$BASE_URL/health" > /dev/null && echo "  ✅ Health OK" || echo "  ❌ Health FAILED"

# Test main page
echo "✓ Testing index.html..."
curl -sf "$BASE_URL/" | grep -q "Simple Release Management" && echo "  ✅ Index OK" || echo "  ❌ Index FAILED"

# Test CSS
echo "✓ Testing CSS files..."
curl -sf "$BASE_URL/css/app.css" > /dev/null && echo "  ✅ CSS OK" || echo "  ❌ CSS FAILED"

# Test JavaScript files
echo "✓ Testing JavaScript files..."
curl -sf "$BASE_URL/js/api.js" > /dev/null && echo "  ✅ api.js OK" || echo "  ❌ api.js FAILED"
curl -sf "$BASE_URL/js/router.js" > /dev/null && echo "  ✅ router.js OK" || echo "  ❌ router.js FAILED"
curl -sf "$BASE_URL/js/app.js" > /dev/null && echo "  ✅ app.js OK" || echo "  ❌ app.js FAILED"
curl -sf "$BASE_URL/js/components/forms.js" > /dev/null && echo "  ✅ forms.js OK" || echo "  ❌ forms.js FAILED"
curl -sf "$BASE_URL/js/components/bundle-wizard.js" > /dev/null && echo "  ✅ bundle-wizard.js OK" || echo "  ❌ bundle-wizard.js FAILED"

# Test API endpoints
echo "✓ Testing API endpoints..."
curl -sf "$BASE_URL/api/v1/tenants" > /dev/null && echo "  ✅ Tenants API OK" || echo "  ❌ Tenants API FAILED"
curl -sf "$BASE_URL/api/v1/bundles" > /dev/null && echo "  ✅ Bundles API OK" || echo "  ❌ Bundles API FAILED"
curl -sf "$BASE_URL/api/v1/releases" > /dev/null && echo "  ✅ Releases API OK" || echo "  ❌ Releases API FAILED"

echo ""
echo "🎉 Web UI test completed!"
echo "📍 Visit: $BASE_URL"
