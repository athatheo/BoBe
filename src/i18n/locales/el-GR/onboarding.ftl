onboarding-local-tier-small-label = Μικρό (4B)
onboarding-local-tier-small-description = Γρήγορο, με χαμηλή κατανάλωση πόρων. Καλό για σύντομες αλληλεπιδράσεις.
onboarding-local-tier-medium-label = Μεσαίο (8B)
onboarding-local-tier-medium-description = Ισορροπημένη απόδοση και ποιότητα.
onboarding-local-tier-large-label = Μεγάλο (14B)
onboarding-local-tier-large-description = Η καλύτερη ποιότητα, απαιτεί περισσότερους πόρους.

onboarding-cloud-provider-openai-label = OpenAI
onboarding-cloud-provider-azure-openai-label = Azure OpenAI
onboarding-cloud-provider-openai-model-gpt-5-mini-label = GPT-5 Mini
onboarding-cloud-provider-openai-model-gpt-5-nano-label = GPT-5 Nano
onboarding-cloud-provider-openai-model-gpt-5-2-label = GPT-5.2

setup-error-create-data-directory = Δεν είναι δυνατή η δημιουργία του φακέλου δεδομένων: { $error }
setup-error-not-enough-disk-space = Δεν υπάρχει αρκετός χώρος στον δίσκο: απαιτούνται ~{ $needed_gb } GB, διαθέσιμα { $available_gb } GB
setup-error-unknown-provider = Άγνωστος πάροχος: { $provider }
setup-error-unknown-mode = Άγνωστη λειτουργία: { $mode }
setup-error-job-not-found = Η εργασία ρύθμισης '{ $job_id }' δεν βρέθηκε
setup-error-persist-failed = Αποτυχία αποθήκευσης της ρύθμισης

setup-step-validate-data-directory-ready = Ο φάκελος δεδομένων είναι έτοιμος
setup-step-engine-ollama-at = Το Ollama βρίσκεται στο { $path }
setup-step-model-pulling = Γίνεται λήψη του μοντέλου { $model }
setup-step-model-ready = Το μοντέλο { $model } είναι έτοιμο
setup-step-vision-model-pull-failed-non-fatal = Η λήψη του μοντέλου όρασης απέτυχε (μη κρίσιμο): { $error }
setup-step-embedding-loading = Φόρτωση του μοντέλου ενσωματώσεων στη μνήμη...
setup-step-embedding-loaded = Το μοντέλο ενσωματώσεων φορτώθηκε
setup-step-embedding-warmup-failed-non-fatal = Η προθέρμανση απέτυχε (μη κρίσιμο): { $error }
setup-step-persist-saved = Η ρύθμιση αποθηκεύτηκε

setup-openai-error-api-key-required = Απαιτείται κλειδί API για το OpenAI
setup-openai-validation-api-key-valid = Το κλειδί API είναι έγκυρο
setup-openai-error-validation-http = Αποτυχία επαλήθευσης κλειδιού API: HTTP { $status }
setup-openai-error-invalid-api-key-format = Μη έγκυρη μορφή κλειδιού API OpenAI. Αφαιρέστε κενά/αλλαγές γραμμής και δοκιμάστε ξανά.
setup-openai-error-cannot-reach = Δεν είναι δυνατή η σύνδεση με το OpenAI: { $error }
setup-openai-embedding-testing = Έλεγχος τελικού σημείου ενσωματώσεων...
setup-openai-embedding-working = Το τελικό σημείο ενσωματώσεων λειτουργεί
setup-openai-embedding-failed = Η δοκιμή ενσωματώσεων απέτυχε: { $error }

setup-azure-error-api-key-required = Απαιτείται κλειδί API
setup-azure-error-endpoint-required = Απαιτείται τελικό σημείο
setup-azure-error-deployment-required = Απαιτείται όνομα ανάπτυξης
setup-azure-validation-endpoint-validated = Το τελικό σημείο Azure επαληθεύτηκε
setup-azure-error-validation-http = Η επαλήθευση Azure απέτυχε: HTTP { $status }
setup-azure-error-invalid-value-format = Μη έγκυρη μορφή τιμής ρύθμισης Azure. Αφαιρέστε κενά/αλλαγές γραμμής και δοκιμάστε ξανά.
setup-azure-error-cannot-reach = Δεν είναι δυνατή η σύνδεση με το τελικό σημείο Azure: { $error }
setup-azure-embedding-testing = Έλεγχος ενσωματώσεων...
setup-azure-embedding-working = Οι ενσωματώσεις λειτουργούν
setup-azure-embedding-failed = Ο έλεγχος ενσωματώσεων απέτυχε: { $error }
