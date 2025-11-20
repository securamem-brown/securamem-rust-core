#!/bin/bash
# SecuraMem Defense Kit - Demo Script 2: Audit Chain Verification
# This script demonstrates cryptographic verification of the immutable audit trail

set -e

echo "╔═══════════════════════════════════════════════════════════════╗"
echo "║  SecuraMem Audit Chain Verification                           ║"
echo "╚═══════════════════════════════════════════════════════════════╝"
echo ""

# Step 1: Verify chain integrity
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "STEP 1: Cryptographic Chain Verification"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Running: smem verify"
echo ""

smem verify

echo ""
echo "✅ All SHA-256 hashes verified"
echo "✅ All Ed25519 signatures valid"
echo "✅ Chain-of-custody intact"
echo ""

sleep 2

# Step 2: Show recent audit entries
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "STEP 2: Recent Audit Entries"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Running: smem audit-log --limit 5"
echo ""

smem audit-log --limit 5

echo ""

sleep 2

# Step 3: Filter firewall decisions
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "STEP 3: Firewall Decision Log"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Running: smem audit-log --filter firewall_decision --limit 10"
echo ""

smem audit-log --filter firewall_decision --limit 10

echo ""

sleep 2

# Step 4: Export audit trail
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "STEP 4: Export Audit Trail (Compliance Report)"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Running: smem export-audit --output audit_report.json"
echo ""

smem export-audit --output audit_report.json

echo ""
echo "✅ Audit report exported: audit_report.json"
echo ""

# Show sample of the report
if [ -f audit_report.json ]; then
    echo "Sample (first 3 entries):"
    echo ""
    cat audit_report.json | jq '.entries[0:3]'
fi

echo ""

sleep 2

# Step 5: Show audit statistics
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "STEP 5: Audit Statistics"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Query the database directly for stats
if [ -f .securamem/memory.db ]; then
    echo "Total audit entries:"
    sqlite3 .securamem/memory.db "SELECT COUNT(*) FROM audit_log;"
    echo ""

    echo "Entries by operation type:"
    sqlite3 .securamem/memory.db "SELECT operation_type, COUNT(*) as count FROM audit_log GROUP BY operation_type ORDER BY count DESC;"
    echo ""

    echo "Firewall decisions:"
    sqlite3 .securamem/memory.db "SELECT
        json_extract(audit_data, '$.decision') as decision,
        COUNT(*) as count
    FROM audit_log
    WHERE operation_type = 'firewall_decision'
    GROUP BY decision;"
    echo ""

    echo "Average similarity score for blocked requests:"
    sqlite3 .securamem/memory.db "SELECT
        AVG(CAST(json_extract(audit_data, '$.similarity_score') AS REAL)) as avg_similarity
    FROM audit_log
    WHERE operation_type = 'firewall_decision'
    AND json_extract(audit_data, '$.decision') = 'BLOCK';"
    echo ""
fi

sleep 2

# Step 6: Demonstrate tamper detection
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "STEP 6: Tamper Detection Demo (Optional)"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "To demonstrate tamper detection:"
echo ""
echo "  1. Manually modify .securamem/memory.db with a SQLite editor"
echo "  2. Run: smem verify"
echo "  3. Expected: Hash chain validation will FAIL"
echo ""
echo "This proves the blockchain-style immutability of the audit trail."
echo ""

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "VERIFICATION COMPLETE"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Key Findings:"
echo "  ✅ All audit entries cryptographically signed (Ed25519)"
echo "  ✅ Hash chain intact (SHA-256 chaining)"
echo "  ✅ Firewall decisions recorded with similarity scores"
echo "  ✅ Audit trail exportable for SOC 2 / HIPAA compliance"
echo ""
echo "Next Steps:"
echo "  - Share audit_report.json with auditors"
echo "  - Integrate with SIEM (syslog forwarding)"
echo "  - Configure Prometheus monitoring"
echo ""
