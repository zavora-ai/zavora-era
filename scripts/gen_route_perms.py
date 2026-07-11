#!/usr/bin/env python3
"""Generate the declarative route->permission registry (middleware/route_perms.rs)
from the parsed protected-route list (/tmp/protected_routes.txt) and the curated
handler->permission map below. Every handler MUST be mapped; unmapped handlers
are reported and abort generation (so the audit matrix is always complete)."""
import re, sys

# handler (routes::mod::fn, without the routes:: prefix) -> permission key
#   "resource.verb"  -> require that catalog permission
#   "SELF"           -> authenticated + self/membership-scoped (no extra perm)
HANDLER_PERM = {
    "dashboard::summary": "SELF",
    # Accounting
    "accounts::list": "account.read", "accounts::create": "account.create",
    "accounts::get": "account.read", "accounts::update": "account.update",
    "accounts::seed": "account.create",
    "periods::list": "period.read", "periods::generate": "period.close",
    "periods::close": "period.close", "periods::reopen": "period.close",
    "periods::year_end_close": "period.close",
    "journal::list": "journal.read", "journal::create": "journal.post",
    "journal::validate": "journal.read", "journal::get": "journal.read",
    "journal::reverse": "journal.reverse",
    "recurring_journals::list": "recurring_journal.read", "recurring_journals::save": "recurring_journal.create",
    "recurring_journals::run_now": "recurring_journal.run", "recurring_journals::delete": "recurring_journal.delete",
    "onboarding::post_opening_balances": "opening_balance.create",
    "dimensions::list": "dimension.read", "dimensions::create_type": "dimension.create",
    "dimensions::create_value": "dimension.create",
    "posting_groups::get_all": "posting_group.read", "posting_groups::create_group": "posting_group.config",
    "posting_groups::assign": "posting_group.config", "posting_groups::upsert_business_control": "posting_group.config",
    "posting_groups::upsert_general": "posting_group.config", "posting_groups::upsert_vat": "posting_group.config",
    # Parties
    "parties::list_customers": "customer.read", "parties::create_customer": "customer.create",
    "parties::get_customer": "customer.read", "parties::update_customer": "customer.update",
    "parties::customer_statement": "customer_statement.read", "parties::send_statement": "customer_statement.send",
    "parties::list_vendors": "vendor.read", "parties::create_vendor": "vendor.create",
    "parties::get_vendor": "vendor.read", "parties::update_vendor": "vendor.update",
    "parties::list_employees": "employee.read", "parties::create_employee": "employee.create",
    "parties::get_employee": "employee.read", "parties::update_employee": "employee.update",
    # Catalog / inventory
    "catalog::list_products": "product.read", "catalog::create_product": "product.create",
    "catalog::get_product": "product.read", "catalog::update_product": "product.update",
    "catalog::delete_product": "product.delete",
    "inventory::list": "inventory.read", "inventory::create": "product.create",
    "inventory::receive": "inventory.receive", "inventory::issue": "inventory.issue",
    "inventory::adjust": "inventory.adjust",
    # Sales
    "invoices::list": "invoice.read", "invoices::create": "invoice.create",
    "invoices::get_one": "invoice.read", "invoices::update": "invoice.update",
    "invoices::delete": "invoice.delete", "invoices::document": "invoice.read",
    "invoices::post_invoice": "invoice.post", "invoices::send": "invoice.send",
    "invoices::write_off": "invoice.void", "invoices::create_credit_note": "credit_note.create",
    "invoices::etims_transmit": "invoice.send",
    "invoices::list_recurring": "recurring_invoice.read", "invoices::create_recurring": "recurring_invoice.create",
    "invoices::update_recurring": "recurring_invoice.update", "invoices::delete_recurring": "recurring_invoice.delete",
    "invoices::recurring_document": "recurring_invoice.read", "invoices::recurring_history": "recurring_invoice.read",
    "invoice_templates::list": "settings.read", "invoice_templates::create": "settings.config",
    "estimates::list": "estimate.read", "estimates::create": "estimate.create",
    "estimates::get_one": "estimate.read", "estimates::update": "estimate.update",
    "estimates::delete": "estimate.delete", "estimates::document": "estimate.read",
    "estimates::convert": "estimate.convert", "estimates::send": "estimate.send",
    "estimates::accept": "estimate.update", "estimates::decline": "estimate.update",
    # Purchases
    "bills::list": "bill.read", "bills::create": "bill.create", "bills::get_one": "bill.read",
    "bills::update": "bill.update", "bills::delete": "bill.delete",
    "bills::approve": "bill.approve", "bills::post_bill": "bill.post",
    "supplier_credit_notes::list": "supplier_credit.read", "supplier_credit_notes::create": "supplier_credit.create",
    "supplier_credit_notes::get_one": "supplier_credit.read",
    "receipts::capture": "bill.create", "receipts::confirm": "bill.create",
    # Procurement
    "procurement::list_applications": "vendor_application.read",
    "procurement::approve_application": "vendor_application.approve",
    "procurement::reject_application": "vendor_application.reject",
    "procurement::list_tenders": "tender.read", "procurement::create_tender": "tender.create",
    "procurement::get_tender": "tender.read", "procurement::publish_tender": "tender.publish",
    "procurement::list_bids": "tender.read", "procurement::award_tender": "tender.award",
    "procurement::analytics": "purchase_order.read", "procurement::budget_control": "purchase_order.read",
    "procurement::list_debit_notes": "debit_note.read", "procurement::create_debit_note": "debit_note.create",
    "procurement::get_debit_note": "debit_note.read",
    "procurement::list_expense_claims": "expense_claim.read", "procurement::create_expense_claim": "expense_claim.create",
    "procurement::get_expense_claim": "expense_claim.read", "procurement::submit_expense_claim": "expense_claim.submit",
    "procurement::approve_expense_claim": "expense_claim.approve", "procurement::reject_expense_claim": "expense_claim.approve",
    "procurement::list_requisitions": "requisition.read", "procurement::create_requisition": "requisition.create",
    "procurement::get_requisition": "requisition.read", "procurement::submit_requisition": "requisition.submit",
    "procurement::approve_requisition": "requisition.approve", "procurement::reject_requisition": "requisition.reject",
    "procurement::convert_requisition": "requisition.convert",
    "procurement::list_purchase_orders": "purchase_order.read", "procurement::create_purchase_order": "purchase_order.create",
    "procurement::get_purchase_order": "purchase_order.read", "procurement::purchase_order_document": "purchase_order.read",
    "procurement::send_purchase_order": "purchase_order.send", "procurement::list_goods_receipts": "goods_receipt.read",
    "procurement::create_goods_receipt": "goods_receipt.create", "procurement::purchase_order_match": "purchase_order.read",
    "approval::list": "approval_limit.read", "approval::set": "approval_limit.config",
    # Banking
    "payments::list": "payment.read", "payments::record": "payment.create",
    "payments::get_one": "payment.read", "payments::apply_unapplied": "payment.apply",
    "payments::mpesa_stk_push": "payment.create",
    "transactions::list": "bank_transaction.read", "transactions::categorise": "bank_transaction.categorise",
    "transactions::split": "bank_transaction.categorise", "transactions::merge": "bank_transaction.categorise",
    "transactions::exclude": "bank_transaction.categorise",
    "bank::list_accounts": "bank_account.read", "bank::create_account": "bank_account.create",
    "bank::delete_account": "bank_account.delete", "bank::import_statement": "bank_transaction.import",
    "bank::extract_statement": "bank_transaction.import", "bank::reconcile": "bank_transaction.reconcile",
    "bank::confirm_match": "bank_transaction.reconcile",
    "reconciliation::list": "reconciliation.read", "reconciliation::compute": "reconciliation.run",
    "reconciliation::complete": "reconciliation.complete",
    # Payroll
    "payroll::list": "pay_run.read", "payroll::run": "pay_run.create", "payroll::detail": "pay_run.read",
    "payroll::delete_draft": "pay_run.delete", "payroll::recompute": "pay_run.create",
    "payroll::list_inputs": "pay_run.read", "payroll::add_input": "pay_run.create",
    "payroll::delete_input": "pay_run.create", "payroll::approve": "pay_run.approve",
    "payroll::post_run": "pay_run.post", "payroll::mark_paid": "pay_run.pay", "payroll::payslip_pdf": "pay_run.read",
    "payroll_masters::list_earning_types": "payroll_config.read", "payroll_masters::create_earning_type": "payroll_config.config",
    "payroll_masters::set_earning_type_active": "payroll_config.config", "payroll_masters::list_deduction_types": "payroll_config.read",
    "payroll_masters::create_deduction_type": "payroll_config.config", "payroll_masters::set_deduction_type_active": "payroll_config.config",
    "payroll_masters::list_departments": "payroll_config.read", "payroll_masters::create_department": "payroll_config.config",
    "payroll_masters::list_statutory": "payroll_config.read", "payroll_masters::upsert_statutory": "payroll_config.config",
    "payroll_masters::list_recurring": "payroll_config.read", "payroll_masters::create_recurring": "payroll_config.config",
    "payroll_masters::delete_recurring": "payroll_config.config", "payroll_masters::list_loans": "payroll_config.read",
    "payroll_masters::create_loan": "payroll_config.config",
    # HR: leave, onboarding
    "leave::list_types": "leave_type.read", "leave::create_type": "leave_type.config",
    "leave::set_type_active": "leave_type.config", "leave::list_holidays": "holiday.read",
    "leave::create_holiday": "holiday.config", "leave::delete_holiday": "holiday.config",
    "leave::list_balances": "leave.read", "leave::list_requests": "leave.read",
    "leave::create_request": "leave.create", "leave::approve": "leave.approve",
    "leave::decline": "leave.approve", "leave::calendar": "leave.read",
    "leave::invite_ess": "portal_invite.create",
    "hr_onboarding::list": "onboarding.read", "hr_onboarding::create": "onboarding.create",
    "hr_onboarding::get_one": "onboarding.read", "hr_onboarding::complete": "onboarding.update",
    "hr_onboarding::set_task": "onboarding.update",
    # CRM
    "crm::get_settings": "crm.read", "crm::set_enabled": "crm.config",
    "crm::list_pipelines": "crm.read", "crm::list_stages": "crm.read", "crm::analytics": "crm.read",
    "crm::list_leads": "lead.read", "crm::create_lead": "lead.create", "crm::update_lead": "lead.update",
    "crm::convert_lead": "lead.convert", "crm::list_opportunities": "opportunity.read",
    "crm::create_opportunity": "opportunity.create", "crm::move_opportunity": "opportunity.update",
    "crm::win_opportunity": "opportunity.close", "crm::lose_opportunity": "opportunity.close",
    "crm::list_activities": "activity.read", "crm::create_activity": "activity.create",
    "crm::complete_activity": "activity.update", "crm::list_tickets": "ticket.read",
    "crm::get_ticket": "ticket.read", "crm::reply_ticket": "ticket.update",
    "crm::set_ticket_status": "ticket.update", "crm::invite_customer": "portal_invite.create",
    # Assets, FX, Tax, eTIMS
    "assets::list": "asset.read", "assets::create": "asset.create", "assets::run_depreciation": "asset.run",
    "fx::list": "fx_rate.read", "fx::upsert": "fx_rate.create", "fx::delete": "fx_rate.delete", "fx::revaluation": "fx_rate.run",
    "tax_filings::list": "tax_filing.read", "tax_filings::file": "tax_filing.create", "tax_filings::remit": "tax_filing.remit",
    "wht::list": "wht_rate.read", "wht::update": "wht_rate.config",
    "etims::get_config": "etims.read", "etims::save_config": "etims.config", "etims::initialize": "etims.config",
    "etims::transmit": "etims.run", "etims::register_product": "etims.run",
    # POS
    "pos::current_session": "pos_session.read", "pos::list_sessions": "pos_session.read",
    "pos::open_session": "pos_session.run", "pos::complete_sale": "pos_sale.create",
    "pos::z_report": "pos_session.read", "pos::close_session": "pos_session.run", "pos::receipt": "pos_sale.read",
    # Reports (content-gated in the handler: payroll->pay_run.read else report.read)
    "reports::generate": "SELF", "reports::export": "SELF",
    "budgets::list": "budget.read", "budgets::set": "budget.config",
    "custom_reports::list": "custom_report.read", "custom_reports::save": "custom_report.create",
    "custom_reports::get": "custom_report.read", "custom_reports::delete": "custom_report.delete",
    "custom_reports::run": "custom_report.read",
    "report_schedules::list": "report_schedule.read", "report_schedules::save": "report_schedule.create",
    "report_schedules::delete": "report_schedule.delete",
    "consolidation::my_entities": "SELF", "consolidation::trial_balance": "SELF",
    # Warehousing (reuses inventory perms)
    "warehouses::list": "inventory.read", "warehouses::create": "inventory.adjust",
    "warehouses::update": "inventory.adjust", "warehouses::transfer": "inventory.adjust",
    "warehouses::stock_in_warehouse": "inventory.read", "warehouses::item_stock": "inventory.read",
    # Manufacturing (reuses inventory perms)
    "manufacturing::list_boms": "inventory.read", "manufacturing::get_bom": "inventory.read",
    "manufacturing::create_bom": "inventory.adjust", "manufacturing::update_bom": "inventory.adjust",
    "manufacturing::list_work_orders": "inventory.read", "manufacturing::get_work_order": "inventory.read",
    "manufacturing::create_work_order": "inventory.adjust", "manufacturing::start_work_order": "inventory.adjust",
    "manufacturing::complete_work_order": "inventory.adjust", "manufacturing::cancel_work_order": "inventory.adjust",
    # Projects (job/project accounting)
    "projects::list": "project.read", "projects::get_one": "project.read",
    "projects::create": "project.manage", "projects::update": "project.manage",
    "projects::summary": "project.read",
    # Notifications (own inbox = SELF; admin delivery/providers = notification_provider)
    "notifications::list": "SELF", "notifications::unread_count": "SELF",
    "notifications::mark_all_read": "SELF", "notifications::mark_read": "SELF",
    "notifications::delivery_list": "notification_provider.read", "notifications::delivery_stats": "notification_provider.read",
    "notifications::get_settings": "notification_provider.read", "notifications::update_settings": "notification_provider.config",
    "notifications::get_providers": "notification_provider.read", "notifications::put_provider": "notification_provider.config",
    "notifications::test_provider": "notification_provider.config",
    # Attachments, agent, settings, admin
    "attachments::upload": "attachment.create", "attachments::list": "attachment.read",
    "attachments::get_one": "attachment.read", "attachments::delete": "attachment.delete",
    "agent::post_from_agent": "journal.post", "agent::run_report": "report.read",
    "settings::get": "settings.read", "settings::update": "settings.config",
    "audit::query": "audit.read", "audit::for_object": "audit.read",
    "users::list": "user.read", "users::create": "user.manage", "users::update": "user.manage",
    "users::resend_invite": "user.manage", "users::my_permissions": "SELF",
    "roles::list": "role.read", "roles::create": "role.create", "roles::detail": "role.read",
    "roles::update": "role.update", "roles::delete": "role.delete", "roles::list_permissions": "role.read",
    "auth_tenants::list_tenants": "SELF", "auth_tenants::create_tenant": "SELF",
    "auth_tenants::switch_tenant": "SELF", "auth_tenants::archive_tenant": "SELF",
    "auth_tenants::unarchive_tenant": "SELF", "auth_tenants::leave_tenant": "SELF",
}

rows = []
unmapped = set()
for line in open("/tmp/protected_routes.txt"):
    line = line.strip()
    if not line or line.startswith("#"):
        continue
    parts = line.split()
    method, path, handler = parts[0], parts[1], parts[2]
    key = handler.replace("routes::", "")
    perm = HANDLER_PERM.get(key)
    if perm is None:
        unmapped.add(key)
        continue
    access = "Access::SelfScoped" if perm == "SELF" else f'Access::Perm("{perm}")'
    rows.append((method, path, access))

if unmapped:
    print("UNMAPPED HANDLERS (add to HANDLER_PERM):", file=sys.stderr)
    for h in sorted(unmapped):
        print("  " + h, file=sys.stderr)
    sys.exit(1)

# de-dup identical rows, keep order
seen = set(); uniq = []
for r in rows:
    if r not in seen:
        seen.add(r); uniq.append(r)

out = []
out.append("//! AUTO-GENERATED by scripts/gen_route_perms.py — do not edit by hand.")
out.append("//! The declarative access-control matrix: every protected route -> the")
out.append("//! permission it requires. Enforced centrally (default-deny) by")
out.append("//! `enforce_permissions`. Regenerate after adding/removing routes.")
out.append("")
out.append("/// What a route requires. `Perm` = a catalog permission key; `SelfScoped`")
out.append("/// = authenticated + row/membership-scoped in the handler (no extra grant).")
out.append("#[derive(Clone, Copy, Debug)]")
out.append("pub enum Access {")
out.append("    Perm(&'static str),")
out.append("    SelfScoped,")
out.append("}")
out.append("")
out.append("/// (HTTP method, matched path pattern, required access).")
out.append("pub const ROUTE_PERMISSIONS: &[(&str, &str, Access)] = &[")
for (m, p, a) in uniq:
    out.append(f'    ("{m}", "{p}", {a}),')
out.append("];")
out.append("")
open("zavora-erp-api/src/middleware/route_perms.rs", "w").write("\n".join(out) + "\n")
print(f"generated {len(uniq)} route-permission rows")
