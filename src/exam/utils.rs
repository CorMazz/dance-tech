// use std::collections::HashMap;

// /// Parses the test form data, which should have a format of a hashmap more or less like this
// ///
// /// Takes in a test_template which it will then mutate, adding the results so that it is graded.
// pub fn parse_test_form_data(test: HashMap<String, String>, mut test_template: Test, proctor: Option<Proctor>) -> Result<Test, TestError> {
//
//     let mut user_info = HashMap::new();
//
//     // Sort the keys so that the graded test gets reconstructed in the same order as the test definition
//     let mut sorted_keys: Vec<&String> = test.keys().collect();
//     sorted_keys.sort();
//
//     for key in sorted_keys {
//         let value = &test[key];
//
//         // Build the hash map with all of the graded items
//         if key.starts_with("table_index") {
//             let key_parts: Vec<&str> = key.split("---").collect();
//             let value_parts: Vec<&str> = value.split("---").collect();
//
//             match (key_parts.len(), value_parts.len()) {
//                 (8, 4) => {
//                     match (
//                         key_parts[1].parse::<usize>(),
//                         key_parts[3].parse::<usize>(),
//                         key_parts[5].parse::<usize>(),
//                         key_parts[7].parse::<usize>(),
//                         value_parts[1].parse::<usize>(),
//                         value_parts[3].parse::<i32>()
//                     ) {
//                         (Ok(table_index), Ok(section_index), Ok(item_index), Ok(scoring_category_index), Ok(scoring_category_label_index), Ok(points)) => {
//
//                             let scoring_category_name = test_template.tables[table_index]
//                                 .sections[section_index]
//                                 .scoring_categories[scoring_category_index].name.clone();
//
//                             let label = test_template.tables[table_index]
//                                 .sections[section_index]
//                                 .scoring_categories[scoring_category_index]
//                                 .values[scoring_category_label_index]
//                                 .clone();
//
//                             if let Some(item) = test_template.tables[table_index]
//                             .sections[section_index]
//                             .competencies
//                             .get_mut(item_index)
//                         {
//                             item.achieved_scores.get_or_insert_with(Vec::new).push(points);
//
//                             item.achieved_score_labels
//                                 .get_or_insert_with(Vec::new)
//                                 .push(AchievedScoreLabel {
//                                      scoring_category_name,
//                                      value: label,
//                                     });
//                         }
//                         },
//
//                         (Err(e), _, _, _, _, _) => return Err(TestError::InternalServerError(format!("Failed to parse table index key '{}': {:?}", key, e))),
//                         (_, Err(e), _, _, _, _) => return Err(TestError::InternalServerError(format!("Failed to parse section index from key '{}': {:?}", key, e))),
//                         (_, _, Err(e), _, _, _) => return Err(TestError::InternalServerError(format!("Failed to parse item index from key'{}': {:?}", key, e))),
//                         (_, _, _, Err(e), _, _) => return Err(TestError::InternalServerError(format!("Failed to parse scoring category index from key '{}': {:?}", key, e))),
//                         (_, _, _, _, Err(e), _) => return Err(TestError::InternalServerError(format!("Failed to parse scoring category label index from value '{}': {:?}", value, e))),
//                         (_, _, _, _, _, Err(e)) => return Err(TestError::InternalServerError(format!("Failed to parse score from value '{}': {:?}", value, e))),
//                     }
//                 }
//                 _ => return Err(TestError::InternalServerError(format!("The key '{}' and value '{}' should be formatted as follows 'table_index---0---section_index---0---item_index---0---scoring_category_index---1': 'scoring_category_value_index---0---points---1'", key, value))),
//             }
//         } else if key.starts_with("bonus_index") {
//             if let Some(bonus_items) = &mut test_template.bonus_items {
//                 let key_parts: Vec<&str> = key.split("---").collect();
//                 match key_parts.len() {
//                     2 => {
//                         match (key_parts[1].parse::<usize>(), value.parse::<i64>()) {
//                             (Ok(bonus_index), Ok(_)) => {
//                                 let _ = bonus_items[bonus_index].achieved.insert(true);
//                             },
//                             (Err(e), _) => return Err(TestError::InternalServerError(format!("Failed to parse bonus index from key '{}': {:?}", key, e))),
//                             (_, Err(e)) => return Err(TestError::InternalServerError(format!("Failed to parse points from value '{}': {:?}", value, e))),
//                         }
//                     }
//                     _ => return Err(TestError::InternalServerError(format!("The key '{}' should be formatted as 'bonus_index---<index>', but got '{}'", key, key))),
//                 }
//             }
//         } else {
//             user_info.insert(key.clone(), value.clone());
//         }
//     }
//
//     // Construct the GradedTestee instance from the user_info hashmap
//     let testee: Testee = match (
//         user_info.get("first_name").cloned(),
//         user_info.get("last_name").cloned(),
//         user_info.get("email").cloned()
//     ) {
//         (Some(first_name), Some(last_name), Some(email)) => Testee {
//             id: None,
//             first_name,
//             last_name,
//             email,
//         },
//         _ => {
//            return Err(TestError::InternalServerError("Missing user information. Please ensure 'first_name', 'last_name', and 'email' are provided.".to_string()));
//         }
//     };
//
//     // Assign the testee
//     test_template.metadata.testee = Some(testee);
//
//     // Grade the test
//     if let Err(e) = test_template.grade() {
//         return Err(TestError::InternalServerError(e)); // Return the error
//     }
//
//     // Assign the proctor
//     test_template.metadata.proctor = proctor;
//
//     Ok(test_template)
//
// }
//
